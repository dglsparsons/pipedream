terraform {
  required_version = "1.6.2"
  backend "s3" {
    bucket         = "dglsparsons-terraform-state"
    key            = "pipedream-project.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "terraform-lock"
  }

  required_providers {
    vercel = {
      source  = "vercel/vercel"
      version = "~> 1.11"
    }
    github = {
      source  = "integrations/github"
      version = "~> 6.2"
    }
  }
}

provider "vercel" {
  team = "stygian-software"
}

provider "github" {
  owner = "dglsparsons"
}

output "vercel_project_id" {
  value = vercel_project.pipedream.id
}

resource "github_repository" "pipedream" {
  name                        = "pipedream"
  description                 = "Pipedream application for managing CI workflows"
  visibility                  = "public"
  has_issues                  = false
  has_discussions             = false
  has_projects                = false
  has_wiki                    = false
  is_template                 = false
  allow_merge_commit          = false
  allow_squash_merge          = true
  allow_rebase_merge          = false
  allow_auto_merge            = true
  squash_merge_commit_title   = "COMMIT_OR_PR_TITLE"
  squash_merge_commit_message = "COMMIT_MESSAGES"
  delete_branch_on_merge      = true
  has_downloads               = false
}

resource "github_actions_secret" "vercel_org_id" {
  repository      = github_repository.pipedream.name
  secret_name     = "VERCEL_ORG_ID"
  plaintext_value = vercel_project.pipedream.team_id
}

resource "github_actions_secret" "vercel_project_id" {
  repository      = github_repository.pipedream.name
  secret_name     = "VERCEL_PROJECT_ID"
  plaintext_value = vercel_project.pipedream.id
}

resource "vercel_project" "pipedream" {
  name = "pipedream"
  git_comments = {
    on_commit       = false
    on_pull_request = false
  }
  preview_comments                                  = false
  prioritise_production_builds                      = true
  serverless_function_region                        = "dub1"
  skew_protection                                   = "12 hours"
  automatically_expose_system_environment_variables = false
}

resource "vercel_project_domain" "pipedream" {
  project_id = vercel_project.pipedream.id
  domain     = "pipedream-ci.vercel.app"
}

resource "vercel_project_environment_variable" "domain" {
  project_id = vercel_project.pipedream.id
  key        = "DOMAIN"
  value      = vercel_project_domain.pipedream.domain
  target     = ["production", "preview"]
}
