use fission::site::{build_site, SiteBuildOptions};

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fission_site_owns_the_home_docs_and_architecture_routes() {
    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output_dir = std::env::temp_dir().join(format!(
        "coil-fission-site-test-{}-{nonce}",
        std::process::id()
    ));
    let _cleanup = RemoveOnDrop(output_dir.clone());
    let mut options = SiteBuildOptions::from_project_dir(&project_dir, "Coil").unwrap();
    options.output_dir = output_dir.clone();
    let report = build_site(&options, &coil_website::site()).unwrap();
    let paths = report
        .routes
        .iter()
        .map(|route| route.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(paths.contains("/"));
    assert!(paths.contains("/docs/intro/"));
    assert!(paths.contains("/architecture/100-fission-native-application-architecture/"));
    assert!(report.routes.len() > 150);

    let home = std::fs::read_to_string(output_dir.join("index.html")).unwrap();
    assert!(home.contains("Build the product"));
    assert!(!home.contains("http-equiv=\"refresh\""));
    assert!(output_dir.join("sitemap.xml").is_file());
    assert!(output_dir.join("search/manifest.json").is_file());
}
