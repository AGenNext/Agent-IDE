# ── Provider / auth (pass-through from root) ──────────────────────────────────
variable "compartment_ocid"    { description = "Compartment OCID" }
variable "ubuntu_image_ocid"   { description = "Ubuntu 22.04 ARM image OCID for your region" }
variable "ssh_public_key_path" { description = "Path to SSH public key"; default = "~/.ssh/id_rsa.pub" }

# ── App ───────────────────────────────────────────────────────────────────────
variable "app_name"         { description = "App name — used for all resource display names" }
variable "domain"           { description = "Public FQDN, e.g. arithmetic.agennext.com" }
variable "container_image"  { description = "OCI image ref, e.g. ghcr.io/org/app:main" }
variable "app_port"         { description = "Port the container listens on"; default = "3000" }

# ── Compute ───────────────────────────────────────────────────────────────────
variable "ocpus"     { description = "ARM vCPUs (free tier max 4)";  default = 2 }
variable "memory_gb" { description = "RAM in GB (free tier max 24)"; default = 12 }

# ── Network ───────────────────────────────────────────────────────────────────
variable "create_vcn"   {
  description = "Create a dedicated VCN. Set false to attach to an existing one."
  default     = true
}
variable "vcn_cidr"    { description = "CIDR for new VCN (create_vcn=true)";    default = "10.0.0.0/16" }
variable "subnet_cidr" { description = "CIDR for new subnet (create_vcn=true)"; default = "10.0.1.0/24" }

# Provided when create_vcn = false
variable "vcn_id"    { description = "Existing VCN OCID";    default = "" }
variable "subnet_id" { description = "Existing subnet OCID"; default = "" }

# Extra TCP ports to open in the security list (beyond 22/80/443)
variable "extra_ports" {
  description = "Additional TCP ports to open inbound"
  type        = list(number)
  default     = []
}

# ── Registry ──────────────────────────────────────────────────────────────────
variable "ghcr_token" { description = "GHCR token for private images"; default = ""; sensitive = true }
variable "ghcr_user"  { description = "GitHub username for GHCR";      default = "" }
