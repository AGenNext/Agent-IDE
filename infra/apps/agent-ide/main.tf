terraform {
  required_providers {
    oci = { source = "oracle/oci"; version = "~> 6.0" }
  }
  required_version = ">= 1.6"
}

provider "oci" {
  tenancy_ocid     = var.tenancy_ocid
  user_ocid        = var.user_ocid
  fingerprint      = var.fingerprint
  private_key_path = var.private_key_path
  region           = var.region
}

module "app" {
  source = "../../modules/oci-app"

  compartment_ocid    = var.compartment_ocid
  ubuntu_image_ocid   = var.ubuntu_image_ocid
  ssh_public_key_path = var.ssh_public_key_path

  app_name        = "agent-ide"
  domain          = "ide.agennext.com"
  container_image = "ghcr.io/agennext/agent-ide:main"
  app_port        = "3000"

  # All inbound traffic through Caddy (443). No raw backend port exposed.
  extra_ports = []

  ghcr_token = var.ghcr_token
  ghcr_user  = var.ghcr_user
}

output "public_ip" { value = module.app.public_ip }
output "ssh"       { value = module.app.ssh }
output "url"       { value = module.app.url }
