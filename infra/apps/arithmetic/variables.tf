variable "tenancy_ocid"       {}
variable "user_ocid"          {}
variable "fingerprint"        {}
variable "private_key_path"   {}
variable "region"             { default = "us-ashburn-1" }
variable "compartment_ocid"   {}
variable "ubuntu_image_ocid"  {}
variable "ssh_public_key_path"{ default = "~/.ssh/id_rsa.pub" }
variable "ghcr_token"         { default = ""; sensitive = true }
variable "ghcr_user"          { default = "" }
