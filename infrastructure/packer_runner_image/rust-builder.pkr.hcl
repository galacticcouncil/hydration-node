
packer {
  required_plugins {
    googlecompute = {
      version = ">= 1.1.1"
      source  = "github.com/hashicorp/googlecompute"
    }
  }
}


source "googlecompute" "rust-builder" {
  project_id   = var.project_id
  source_image = var.source_image
  ssh_username = "ubuntu"
  zone         = var.gcp_zone
  network      = var.gcp_vpc
  subnetwork   = var.gcp_sub
  ssh_timeout  = "1h"
}

build {
  sources = ["sources.googlecompute.rust-builder"]
  provisioner "shell" {
    script = "setup.sh"
  }
}
