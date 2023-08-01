
variable "project_id" {
  type        = string
  description = "GCP Project ID that the build VM will be deployed to"
}

variable "source_image" {
  type        = string
  description = "GCP Image that will be used as source for the Packer custom image"
}

variable "gcp_zone" {
  type        = string
  description = "GCP zone that the VM instance will be deployed to"
}

variable "gcp_vpc" {
  type        = string
  description = "GCP VPC network name that the VM instance will be deployed to"
}

variable "gcp_sub" {
  type        = string
  description = "GCP Subnetwork name that the VM instance will be deployed to"
}

