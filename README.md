# ✂️ Silcut

**Silcut** is a lightweight, native Linux cutting software and controller designed for **Silhouette Cameo** plotters. Built as a free, open-source alternative to Silhouette Studio for the Linux and digital crafting community.

---

## 🚀 Planned Features

- 🐧 **Native Linux Support:** Runs natively without needing Wine or virtual machines.
- 📐 **Vector Support (SVG):** Import standard vector files for cutting.
- ⚙️ **Machine Control:** Adjust blade depth, force/pressure, speed, and number of passes.
- 🔌 **Direct USB Connection:** Native communication with the plotter via USB/udev.
- 🎯 **Print & Cut:** Planned support for registration mark alignment.

---

## 📟 Target Models

- Silhouette Cameo 1 / 2 / 3 / 4 / 5
- *(Other Silhouette models may be added in the future)*

---

## 🛠️ Installation & Setup (Linux)

### 1. USB Rules (`udev`)
To allow `silcut` to communicate with your Silhouette without requiring `root` (`sudo`) permissions, set up the appropriate udev rule:

```bash
# Add your user to the 'plugdev' group (if applicable)
sudo usermod -aG plugdev $USER
