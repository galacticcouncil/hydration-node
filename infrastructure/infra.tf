# Setting Up Remote State
variable "branch_name" {
  description = ""
}
terraform {
  required_version = ">= 0.12.24"

  backend "s3" {
    bucket = "hydradx-ci-backend-state"
    key    = "BRANCH_ACTOR_KEY"
    region = "eu-west-1"
  }
}

provider "aws" {
  region = var.aws_region
}

variable "aws_region" {
  description = "The AWS region to create resources in."
  default     = "eu-west-1"
}

variable "ec2_pwd" {
  description = ""
}


resource "aws_instance" "runner-aws" {
    ami = "ami-0814bae4faec53b79"
    instance_type = "c5ad.4xlarge"
    subnet_id = "subnet-0ba99ac0d4aea3dc6"
    key_name = "aws-ec2-key"
    vpc_security_group_ids = ["sg-05f1a5d51f4d92cae"]

    tags = {
        Type = "Github_Self_Runner"
    }
    connection {
        type = "ssh"
        user = "ubuntu"
        host = aws_instance.runner-aws.public_ip
        password = var.ec2_pwd
        timeout = "3m"
    }
    provisioner "file" {
      source      = "run_conf.sh"
      destination = "/home/ubuntu/run_conf.sh"
    }
    provisioner "remote-exec" {
        inline = [
        "tmux new -d 'bash run_conf.sh ${var.branch_name}'"
        ]
    }
}
