#![forbid(unsafe_code)]

mod home;

use fission::site::FissionSite;

pub fn site() -> FissionSite {
    FissionSite::new()
        .favicon("/img/favicon.svg")
        .content_transform(|source, _project_dir, _source_path| {
            Ok(neutralize_example_link_attributes(source))
        })
        .route_widget::<(), _>(
            "/",
            "Coil — Build the product. Keep the platform coherent.",
            Some(
                "A Fission-native Rust product framework for public, interactive, and operational web surfaces."
                    .to_string(),
            ),
            home::HomePage,
        )
}

fn neutralize_example_link_attributes(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut in_fence = false;
    for line in source.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            output.push_str(line);
        } else if in_fence {
            output.push_str(
                &line
                    .replace("href=\"", "href =\"")
                    .replace("src=\"", "src =\""),
            );
        } else {
            output.push_str(line);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::neutralize_example_link_attributes;

    #[test]
    fn generated_link_audit_does_not_treat_fenced_examples_as_live_links() {
        let markdown =
            "[Live](/docs/)\n```html\n<a href=\"/example\"><img src=\"/image.png\"></a>\n```\n";
        let transformed = neutralize_example_link_attributes(markdown);

        assert!(transformed.contains("[Live](/docs/)"));
        assert!(transformed.contains("href =\"/example\""));
        assert!(transformed.contains("src =\"/image.png\""));
    }
}
