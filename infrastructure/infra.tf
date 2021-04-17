# Setting Up Remote State
terraform {
  # Terraform version at the time of writing this post
  required_version = ">= 0.12.24"

  #backend "s3" {
    #bucket = "hydradx-ci-backend-state"
    #key    = "example-key" #Variable coming from the CI
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

variable "ec2_secret" {
  description = "The name of the branch that's being deployed"
}

resource "aws_instance" "runner-aws" {
    ami = "ami-05181117cd8d52108"
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
        private_key = var.ec2_secret
        timeout = "3m"
        agent = false
    }

    provisioner "remote-exec" {
        inline = [
        "chmod +x get_token.sh",
        "chmod +x config_script.sh",
        "bash config_script.sh $ACCESS_TOKEN",
        ]
    }
}
