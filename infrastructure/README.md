
This folder contains the infrastructure for deployment of the github runner.

## Github Secrets

The Github pipeline is using some sensitive variables that are not configured in the repository, those are

- GOOGLE_CREDENTIALS
  - GCP service account keys for terraform deployments
- RUNNER_ACCESS_TOKEN
  - Github access token that will authorized the newly deployed Github runner

These variables needs to be configured as a Github secrets or otherwise injected into the pipeline from other secret managers.

## GCP backend bucket configuration

In the `backend.tf` file you need to replace the `__INSERT_GCP_STORAGE_BUCKET_NAME__` variable with the GCP Storage bucket name, where the statefiles of the terraform pipelines will be stored. The prefix of the backend configuration is then generated dynamically during the pipeline steps.

## Terraform parameters

The infrastructure is parametrized and variables needs to be configured before the infrastructure can be used in the deployment pipeline.

The variables are declared in the `variables.tf` file. The values of the parameters can be filled in the `terraform.tfvars` file

example of the content of the file is below:

```
region              = "europe-west1"
project_id          = "test-project-123456"
runner_machine_type = "e2-medium"
runner_zone         = "europe-west1-b"
runner_vpc_name     = "test-vpc"
runner_sub_name     = "test-sub"
runner_image_name   = "packer-1688456837"
```

after configuring the parameters and the secrets, you can run the Github actions pipeline and the VM instance with the runner will be automatically created as well as decommissioned at the end of the pipeline.
