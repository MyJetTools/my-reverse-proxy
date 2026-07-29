fn main() {
    // NB: CiGenerator rewrites Dockerfile (and, with generate_github_ci_file,
    // release.yaml) on every build — hand-edits to either are silently reverted.
    // So extra COPY lines (wwwroot, ip_v6) have to be declared here rather than
    // in the Dockerfile. wwwroot is committed, built by ui/build.sh, so CI needs
    // no extra step.
    ci_utils::ci_generator::CiGenerator::new(env!("CARGO_PKG_NAME"))
        .as_basic_service()
        .generate_github_ci_file()
        .add_docker_copy_file("./wwwroot", "./wwwroot")
        .add_docker_copy_file("./ip_v6", "./ip_v6")
        .build();
}
