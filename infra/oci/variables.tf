variable "tenancy_ocid"       { description = "OCI tenancy OCID" }
variable "user_ocid"          { description = "OCI user OCID" }
variable "fingerprint"        { description = "API key fingerprint" }
variable "private_key_path"   { description = "Path to OCI API private key" }
variable "region"             { description = "OCI region" default = "us-ashburn-1" }
variable "compartment_ocid"   { description = "Compartment OCID" }

variable "app_name"           { description = "App name (used for resource names)" }
variable "domain"             { description = "Public domain, e.g. arithmetic.agennext.com" }
variable "container_image"    { description = "OCI container image, e.g. ghcr.io/unboxd-cloud/arithmetic-platform:main" }
variable "app_port"           { description = "Port the app listens on" default = "3000" }

variable "ubuntu_image_ocid"  { description = "Ubuntu 22.04 ARM image OCID for your region" }
variable "ssh_public_key_path"{ description = "Path to SSH public key" default = "~/.ssh/id_rsa.pub" }
variable "ocpus"              { description = "vCPUs (free tier max 4)" default = 2 }
variable "memory_gb"          { description = "RAM in GB (free tier max 24)" default = 12 }

variable "ghcr_token"         { description = "GitHub token for GHCR pull (if private image)" default = "" sensitive = true }
variable "ghcr_user"          { description = "GitHub username for GHCR" default = "" }
