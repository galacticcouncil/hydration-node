

###############################################################################
###  GCP VM Instance for Github runner
###############################################################################

resource "google_compute_instance" "runner" {
  project      = var.project_id
  name         = "github-runner-${var.runner_id}"
  machine_type = var.runner_machine_type
  zone         = var.runner_zone

  tags         = ["github-runner"]

  # VM uses a custom image built by Packer stored in the same project
  boot_disk {
    initialize_params {
      image = var.runner_image_name
    }
  }

  network_interface {
    network    = var.runner_vpc_name
    subnetwork = var.runner_sub_name
    subnetwork_project = var.project_id

    access_config { # assigns public IP to the intance

    }
  }

  metadata_startup_script = templatefile("${path.module}/files/startup_script.shtpl", {access_token = var.access_token, runner_id = var.runner_id})

#  service_account { # optional: see below
#    email  = google_service_account.runner_sa.email
#    scopes = ["cloud-platform"]
#  }
}


## Optional: Service account for the runner VM instance

#resource "google_service_account" "runner_sa" {
#  account_id   = "github-runner-sa-id"
#  display_name = "Github Runner"
#}
