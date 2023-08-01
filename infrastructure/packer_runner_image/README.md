
# Github Runner Image creation

This folder contains Packer configuration file which is used for automatic creation of GCP Image with preconfigured environment for the Github Runner and any other required dependencies. Configuration file as well as setup script (setup.sh) can be changed and new image can be created.

The image is created by Packer via automatically deployed GCP VM instances. After the VM image is prepared according to the setup script, the VM instance will be automatically decommissioned and the GCP VM Image will be created in the same project. You can then use the name of the Image in you infrastructure pipeline to run the Github Runner.

 In order for the script to work you need to fill the environment variables and then run the build.

## Prepare authentication

In order for the Packer to be able to deploy the GCP VM instance, you need to prepare the authentication method. 

Probably the easiest method for local build is to create a GCP service account, create a service account keys, download those keys to local environment and configure the following environment variable.

```
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/the/json/file
```

## Fill variables

In the `variables.pkrvars.hcl` file you need to fill all the variables declared in the `variables.pkr.hcl` file. These are mostly variables defining the GCP project, network and subnet where the VM will be deployed and configured. 

Example of the filled parameters is here:
```
project_id   = "kot-test-123456"
source_image = "ubuntu-pro-2004-focal-v20230629"
gcp_zone     = "europe-west1-b"
gcp_vpc      = "test-vpc"
gcp_sub      = "test-sub"
```

## Start Packer Build

After the parameters are configured, you can run the packer by following command:

```
packer build -var-file=variable.pkrvars.hcl
```


