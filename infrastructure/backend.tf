# Terraform state storage backend resource

terraform {
  backend "gcs" {
    bucket = "__INSERT_GCP_STORAGE_BUCKET_NAME__"
    prefix = "BRANCH_ACTOR_KEY"
  }
}
