# Setting Up Remote State
terraform {
  # Terraform version at the time of writing this post
  required_version = ">= 0.12.24"

  #backend "s3" {
    #bucket = "example-bucket"
    #key    = "example-key"
    #region = "eu-west-1"
  #}
}

provider "aws" {
  region = var.aws_region
}

variable "aws_region" {
  description = "The AWS region to create resources in."
  default     = "eu-west-1"
}

variable "branch_name" {
  description = "The name of the branch that's being deployed"
}

resource "aws_instance" "runner-aws" {
    name = "runner-aws-${branch_name}"
    ami = "ami-06fd78dc2f0b69910"
    instance_type = "c5ad.4xlarge"
    subnet_id = "subnet-0ba99ac0d4aea3dc6"
    tags {
        Type = "Github_Self_Runner"
    }
    provisioner "file" {
        source      = "config_script.sh"
        destination = "/tmp/config_script.sh"
    }

    provisioner "file" {
        source      = "get_token.sh"
        destination = "/tmp/get_token.sh"
    }
  
    provisioner "remote-exec" {
        inline = [
        "chmod +x /tmp/get_token.sh",
        "chmod +x /tmp/config_script.sh",
        "/tmp/config_script.sh",
        ]
    }
}
