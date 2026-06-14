# Reusable OCI single-instance app module
# Provisions: VCN (optional) + subnet + security list + ARM A1 compute + cloud-init
#
# Usage — own VCN (default):
#   module "arithmetic" {
#     source = "../../modules/oci-app"
#     ...
#   }
#
# Usage — shared VCN:
#   module "arithmetic" {
#     source     = "../../modules/oci-app"
#     create_vcn = false
#     vcn_id     = module.network.vcn_id
#     subnet_id  = module.network.subnet_id
#   }

# ── Network (conditional) ─────────────────────────────────────────────────────

resource "oci_core_vcn" "this" {
  count          = var.create_vcn ? 1 : 0
  compartment_id = var.compartment_ocid
  cidr_blocks    = [var.vcn_cidr]
  display_name   = "${var.app_name}-vcn"
  dns_label      = replace(var.app_name, "-", "")
}

resource "oci_core_internet_gateway" "this" {
  count          = var.create_vcn ? 1 : 0
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.this[0].id
  display_name   = "${var.app_name}-igw"
  enabled        = true
}

resource "oci_core_route_table" "this" {
  count          = var.create_vcn ? 1 : 0
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.this[0].id
  display_name   = "${var.app_name}-rt"

  route_rules {
    destination       = "0.0.0.0/0"
    network_entity_id = oci_core_internet_gateway.this[0].id
  }
}

resource "oci_core_security_list" "this" {
  count          = var.create_vcn ? 1 : 0
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.this[0].id
  display_name   = "${var.app_name}-sl"

  egress_security_rules {
    protocol    = "all"
    destination = "0.0.0.0/0"
    stateless   = false
  }

  ingress_security_rules {
    protocol  = "6"
    source    = "0.0.0.0/0"
    tcp_options { min = 22; max = 22 }
    stateless = false
  }

  ingress_security_rules {
    protocol  = "6"
    source    = "0.0.0.0/0"
    tcp_options { min = 80; max = 80 }
    stateless = false
  }

  ingress_security_rules {
    protocol  = "6"
    source    = "0.0.0.0/0"
    tcp_options { min = 443; max = 443 }
    stateless = false
  }

  dynamic "ingress_security_rules" {
    for_each = var.extra_ports
    content {
      protocol  = "6"
      source    = "0.0.0.0/0"
      tcp_options { min = ingress_security_rules.value; max = ingress_security_rules.value }
      stateless = false
    }
  }
}

resource "oci_core_subnet" "this" {
  count             = var.create_vcn ? 1 : 0
  compartment_id    = var.compartment_ocid
  vcn_id            = oci_core_vcn.this[0].id
  cidr_block        = var.subnet_cidr
  display_name      = "${var.app_name}-subnet"
  dns_label         = "app"
  route_table_id    = oci_core_route_table.this[0].id
  security_list_ids = [oci_core_security_list.this[0].id]
}

locals {
  vcn_id    = var.create_vcn ? oci_core_vcn.this[0].id : var.vcn_id
  subnet_id = var.create_vcn ? oci_core_subnet.this[0].id : var.subnet_id
}

# ── Compute ───────────────────────────────────────────────────────────────────

data "oci_identity_availability_domains" "ads" {
  compartment_id = var.compartment_ocid
}

resource "oci_core_instance" "app" {
  compartment_id      = var.compartment_ocid
  availability_domain = data.oci_identity_availability_domains.ads.availability_domains[0].name
  display_name        = var.app_name
  shape               = "VM.Standard.A1.Flex"

  shape_config {
    ocpus         = var.ocpus
    memory_in_gbs = var.memory_gb
  }

  source_details {
    source_type = "image"
    source_id   = var.ubuntu_image_ocid
  }

  create_vnic_details {
    subnet_id        = local.subnet_id
    assign_public_ip = true
    display_name     = "${var.app_name}-vnic"
  }

  metadata = {
    ssh_authorized_keys = file(var.ssh_public_key_path)
    user_data = base64encode(templatefile("${path.module}/cloud-init.yml.tpl", {
      app_name   = var.app_name
      image      = var.container_image
      domain     = var.domain
      port       = var.app_port
      ghcr_token = var.ghcr_token
      ghcr_user  = var.ghcr_user
    }))
  }
}

# ── Outputs ───────────────────────────────────────────────────────────────────

output "public_ip"   { value = oci_core_instance.app.public_ip }
output "instance_id" { value = oci_core_instance.app.id }
output "ssh"         { value = "ssh ubuntu@${oci_core_instance.app.public_ip}" }
output "url"         { value = "https://${var.domain}" }
output "vcn_id"      { value = local.vcn_id }
output "subnet_id"   { value = local.subnet_id }
