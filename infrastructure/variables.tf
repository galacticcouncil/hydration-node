variable "region" {
  type        = string
  description = "GCP Region that the resources will be deployed in"
}

variable "project_id" {
  type        = string
  description = "GCP Project ID where the resources will be deployed"
}

variable "access_token" {
  type        = string
  description = "Github Access token for generating runner authentication token"
}

variable "runner_id" {
  type        = string
  description = "Unique ID of the self-hosted Github runner deployed in GCP"
}

variable "runner_machine_type" {
  type        = string
  description = "GCP Machine type that the VM instance will use"
}

variable "runner_zone" {
  type        = string
  description = "GCP Zone that the VM instance will be deployed to"
}

variable "runner_vpc_name" {
  type        = string
  description = "Name of the VPC Network that the VM instance will be deployed to"
}

variable "runner_sub_name" {
  type        = string
  description = "Name of the VPC Subnet that the VM instance will be deployed to"
}

variable "runner_image_name" {
  type        = string
  description = "Name of the custom Compute image built by Packer with pre-installed environment for Github runner"
}
