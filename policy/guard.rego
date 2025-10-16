package vaultmesh.guard

default allow := true

deny[msg] {
  input.action == "apply"
  not semver_at_least(input.env.terraform_version, "1.6.0")
  msg := sprintf("Terraform %v < 1.6.0", [input.env.terraform_version])
}

deny["non-GitHub CI not authorized"] {
  input.env.ci != "github_actions"
}

allow {
  count(deny) == 0
}

semver_at_least(v, min) {
  nums := split(split(v, "-")[0], ".")
  mins := split(min, ".")
  to_number(nums[0]) > to_number(mins[0])
} else {
  nums := split(split(v, "-")[0], ".")
  mins := split(min, ".")
  to_number(nums[0]) == to_number(mins[0])
  to_number(nums[1]) >  to_number(mins[1])
} else {
  nums := split(split(v, "-")[0], ".")
  mins := split(min, ".")
  to_number(nums[0]) == to_number(mins[0])
  to_number(nums[1]) == to_number(mins[1])
  to_number(nums[2]) >= to_number(mins[2])
}

