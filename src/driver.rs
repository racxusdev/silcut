//! Transporte USB (libusb via rusb) e comandos de nível de máquina.
//!
//! Endpoints verificados por captura da comunidade: escrita 0x01 (bulk OUT),
//! leitura 0x82 (bulk IN), pacotes de até 4096 bytes; transmissões longas
//! são fracionadas em blocos de 1 KiB alinhados ao terminador 0x03, com
//! espera de "ready" entre blocos (mesmo comportamento do Silhouette Studio).

use std::time::Duration;

use rusb::UsbContext;

use crate::device::{self, DeviceSpec};
use crate::error::{Result, SilError};

const ENDPOINT_OUT: u8 = 0x01;
const ENDPOINT_IN: u8 = 0x82;
const WRITE_CHUNK: usize = 4096;
const SAFE_CHUNK: usize = 1024;
const ETX: u8 = 0x03;

/// Estado do cortador (resposta ao ESC 0x05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,
    Moving,
    Unloaded,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ready => "pronta",
            Status::Moving => "em movimento",
            Status::Unloaded => "sem mídia / tampa aberta",
        }
    }
}

/// Dispositivo avistado no barramento USB.
#[derive(Debug, Clone)]
pub struct ListedDevice {
    pub vid: u16,
    pub pid: u16,
    pub bus: u8,
    pub address: u8,
    pub spec: Option<&'static DeviceSpec>,
}

impl ListedDevice {
    pub fn display_name(&self) -> String {
        match self.spec {
            Some(s) => s.name.to_string(),
            None => format!("Graphtec desconhecido {:04x}:{:04x}", self.vid, self.pid),
        }
    }
}

/// Lista todos os cortadores conhecidos conectados.
pub fn list_devices() -> Result<Vec<ListedDevice>> {
    let ctx = rusb::Context::new()?;
    let devices = ctx.devices()?;
    let mut out = Vec::new();
    for d in devices.iter() {
        let Ok(desc) = d.device_descriptor() else { continue };
        let vid = desc.vendor_id();
        let pid = desc.product_id();
        let known = device::find_by_ids(vid, pid).is_some();
        let graphtec = vid == device::VID_GRAPHTEC || vid == device::VID_CAMEO5_ALPHA;
        if known || graphtec {
            out.push(ListedDevice {
                vid,
                pid,
                bus: d.bus_number(),
                address: d.address(),
                spec: device::find_by_ids(vid, pid),
            });
        }
    }
    Ok(out)
}

/// Cortador aberto e pronto para comandos.
pub struct Cutter {
    handle: rusb::DeviceHandle<rusb::Context>,
    pub spec: &'static DeviceSpec,
    pub location: String,
}

impl Cutter {
    /// Abre o enésimo dispositivo da lista de [`list_devices`].
    pub fn open_listed(listed: &ListedDevice) -> Result<Cutter> {
        let ctx = rusb::Context::new()?;
        let devices = ctx.devices()?;
        let mut target: Option<rusb::Device<rusb::Context>> = None;
        for d in devices.iter() {
            if d.bus_number() == listed.bus && d.address() == listed.address {
                let desc = d.device_descriptor()?;
                if desc.vendor_id() == listed.vid && desc.product_id() == listed.pid {
                    target = Some(d);
                }
                break;
            }
        }
        let dev = target.ok_or(SilError::DeviceNotFound)?;
        let handle = dev.open().map_err(|e| map_usb(e, listed))?;
        // Remove driver de kernel (ex.: usbhid) se necessário.
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(0).map_err(|e| map_usb(e, listed))?;
        Ok(Cutter {
            handle,
            spec: listed.spec.ok_or(SilError::DeviceNotFound)?,
            location: format!("bus {bus:03} addr {addr:03}", bus = listed.bus, addr = listed.address),
        })
    }

    /// Abre o primeiro cortador conhecido. `None` se não houver nenhum.
    pub fn open_first() -> Result<Cutter> {
        let list = list_devices()?;
        let with_spec = list.iter().find(|l| l.spec.is_some());
        let listed = with_spec.or_else(|| list.first()).ok_or(SilError::DeviceNotFound)?;
        Cutter::open_listed(listed)
    }

    // -- baixo nível --------------------------------------------------------

    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let timeout = Duration::from_millis(10_000);
        let mut sent = 0usize;
        let mut retries = 0u32;
        while sent < data.len() {
            let end = (sent + WRITE_CHUNK).min(data.len());
            match self.handle.write_bulk(ENDPOINT_OUT, &data[sent..end], timeout) {
                Ok(0) | Err(rusb::Error::Timeout) => {
                    retries += 1;
                    if retries > 5 {
                        return Err(SilError::Timeout(10));
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                Ok(n) => {
                    retries = 0;
                    sent += n;
                }
                Err(e) => return Err(SilError::Usb(e)),
            }
        }
        Ok(())
    }

    /// Lê uma resposta terminada em 0x03.
    pub fn read_response(&mut self, timeout: Duration) -> Result<String> {
        let mut buf = [0u8; 64];
        let mut acc: Vec<u8> = Vec::with_capacity(64);
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Err(SilError::Timeout(timeout.as_secs().max(1)));
            }
            match self.handle.read_bulk(ENDPOINT_IN, &mut buf, left) {
                Ok(0) => continue,
                Ok(n) => {
                    acc.extend_from_slice(&buf[..n]);
                    if acc.last() == Some(&ETX) {
                        acc.pop();
                        let s = String::from_utf8_lossy(&acc).trim().to_string();
                        return Ok(s);
                    }
                }
                Err(rusb::Error::Timeout) => continue,
                Err(e) => return Err(SilError::Usb(e)),
            }
        }
    }

    // -- comandos -----------------------------------------------------------

    fn send_escape(&mut self, code: u8) -> Result<()> {
        self.write_all(&[0x1b, code])
    }

    /// Comando simples terminado em 0x03 (sem resposta esperada).
    pub fn send_cmd(&mut self, cmd: &str) -> Result<()> {
        let mut buf = Vec::with_capacity(cmd.len() + 1);
        buf.extend_from_slice(cmd.as_bytes());
        buf.push(ETX);
        self.write_all(&buf)
    }

    /// Comando com resposta (`FG`, `TB71`, `FA`, …).
    pub fn query(&mut self, cmd: &str, timeout: Duration) -> Result<String> {
        self.send_cmd(cmd)?;
        self.read_response(timeout)
    }

    /// Status atual.
    pub fn status(&mut self) -> Result<Status> {
        self.send_escape(0x05)?;
        let resp = self.read_response(Duration::from_secs(5))?;
        parse_status(&resp)
    }

    /// Aguarda até `timeout` pelo estado Ready, chamando `progress` com cada estado.
    pub fn wait_ready(
        &mut self,
        timeout: Duration,
        progress: &mut dyn FnMut(&Status),
    ) -> Result<Status> {
        let poll = Duration::from_secs(2);
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let st = self.status()?;
            progress(&st);
            if st == Status::Ready {
                return Ok(st);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(st);
            }
            std::thread::sleep(poll);
        }
    }

    /// Consulta firmware (`FG`).
    pub fn version(&mut self) -> Result<String> {
        self.query("FG", Duration::from_secs(10))
    }

    /// Inicializa o dispositivo (ESC 0x04) e devolve a versão de firmware.
    pub fn initialize(&mut self) -> Result<String> {
        self.send_escape(0x04)?;
        self.wait_ready(Duration::from_secs(30), &mut |_| {})?;
        let v = self.version()?;
        if self.spec.family == device::Family::Cameo3On {
            // Consultas de calibração que o Studio também faz
            let _ = self.query("TB71", Duration::from_secs(5));
            let _ = self.query("FA", Duration::from_secs(5));
        }
        Ok(v)
    }

    /// Envio seguro de fluxo longo: fatia em blocos ≤ 1 KiB terminados em 0x03,
    /// aguardando ready entre blocos (evita sobrecarregar o buffer da máquina).
    pub fn safe_send(&mut self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut start = 0usize;
        while start < data.len() {
            let mut end = (start + SAFE_CHUNK).min(data.len());
            // recua até o último terminador para não cortar comando ao meio
            while end < data.len() && data[end - 1] != ETX {
                end -= 1;
            }
            if end == start {
                end = (start + SAFE_CHUNK).min(data.len());
            }
            self.write_all(&data[start..end])?;
            if end < data.len() {
                self.wait_ready(Duration::from_secs(120), &mut |_| {})?;
            }
            start = end;
        }
        Ok(())
    }

    /// Alimenta a mídia para fora (FO0) — usado na ejeção.
    pub fn feed_out(&mut self) -> Result<()> {
        self.wait_ready(Duration::from_secs(60), &mut |_| {})?;
        self.send_cmd("FO0")
    }
}

fn parse_status(resp: &str) -> Result<Status> {
    match resp.trim() {
        "0" => Ok(Status::Ready),
        "1" => Ok(Status::Moving),
        "2" => Ok(Status::Unloaded),
        other => Err(SilError::Protocol(format!(
            "status inesperado: '{other}'"
        ))),
    }
}

fn map_usb(e: rusb::Error, listed: &ListedDevice) -> SilError {
    match e {
        rusb::Error::Access => SilError::AccessDenied(format!(
            "{:04x}:{:04x} ({}). Instale a regra udev:\n  sudo cp 70-silhouette.rules /etc/udev/rules.d/\n  sudo udevadm control --reload-rules && sudo udevadm trigger",
            listed.vid, listed.pid, listed.display_name()
        )),
        other => SilError::Usb(other),
    }
}
