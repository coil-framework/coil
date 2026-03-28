use crate::SiteDescriptor;
use anyhow::{Result, bail};
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardInput {
    pub name: String,
    pub display_name: String,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub modules: Vec<String>,
    pub extra_sites: Vec<SiteDescriptor>,
}

pub fn run_wizard(target: &Path) -> Result<WizardInput> {
    let theme = ColorfulTheme::default();
    let default_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_slug)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "coil-store".to_string());
    let name: String = Input::with_theme(&theme)
        .with_prompt("Project name")
        .default(default_name.clone())
        .validate_with(|input: &String| -> Result<(), &str> {
            if sanitize_slug(input).is_empty() {
                Err("enter a name using letters, numbers, or hyphens")
            } else {
                Ok(())
            }
        })
        .interact_text()?;
    let name = sanitize_slug(&name);
    let display_name: String = Input::with_theme(&theme)
        .with_prompt("Display name")
        .default(title_case_slug(&name))
        .interact_text()?;

    let locales = ["en-GB", "fr-FR", "pl-PL", "de-DE", "en-US"];
    let default_locale_index = Select::with_theme(&theme)
        .with_prompt("Default locale")
        .items(&locales)
        .default(0)
        .interact()?;
    let default_locale = locales[default_locale_index].to_string();
    let additional_locale_indices = MultiSelect::with_theme(&theme)
        .with_prompt("Additional locales")
        .items(&locales)
        .defaults(
            &locales
                .iter()
                .enumerate()
                .map(|(index, locale)| index != default_locale_index && *locale == "fr-FR")
                .collect::<Vec<_>>(),
        )
        .interact()?;
    let mut supported_locales = vec![default_locale.clone()];
    for index in additional_locale_indices {
        let candidate = locales[index].to_string();
        if candidate != default_locale && !supported_locales.contains(&candidate) {
            supported_locales.push(candidate);
        }
    }

    let module_ids = ["cms", "media", "commerce", "admin", "ops", "memberships", "events"];
    let selected = MultiSelect::with_theme(&theme)
        .with_prompt("Official modules")
        .items(&module_ids)
        .defaults(&[true, true, true, true, true, false, false])
        .interact()?;
    let modules = if selected.is_empty() {
        vec!["cms".to_string(), "commerce".to_string(), "admin".to_string()]
    } else {
        selected
            .into_iter()
            .map(|index| module_ids[index].to_string())
            .collect::<Vec<_>>()
    };

    let mut extra_sites = Vec::new();
    while Confirm::with_theme(&theme)
        .with_prompt("Add another site now?")
        .default(false)
        .interact()?
    {
        let site_id: String = Input::with_theme(&theme)
            .with_prompt("Site id")
            .default(format!("{name}-{}", extra_sites.len() + 2))
            .validate_with(|input: &String| -> Result<(), &str> {
                if sanitize_slug(input).is_empty() {
                    Err("enter a site id using letters, numbers, or hyphens")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;
        let site_id = sanitize_slug(&site_id);
        let site_display_name: String = Input::with_theme(&theme)
            .with_prompt("Site display name")
            .default(title_case_slug(&site_id))
            .interact_text()?;
        let site_brand_name: String = Input::with_theme(&theme)
            .with_prompt("Brand name")
            .default(site_display_name.clone())
            .interact_text()?;
        let default_domain: String = Input::with_theme(&theme)
            .with_prompt("Canonical domain")
            .default(format!("{site_id}.localhost"))
            .interact_text()?;
        let locale_index = Select::with_theme(&theme)
            .with_prompt("Default locale for this site")
            .items(&supported_locales)
            .default(0)
            .interact()?;
        let site_default_locale = supported_locales[locale_index].clone();
        extra_sites.push(SiteDescriptor {
            id: site_id,
            display_name: site_display_name,
            brand_name: site_brand_name,
            canonical_domain: default_domain,
            additional_domains: Vec::new(),
            default_locale: site_default_locale.clone(),
            supported_locales: vec![site_default_locale],
        });
    }

    if modules.is_empty() {
        bail!("at least one module must be selected");
    }

    Ok(WizardInput {
        name,
        display_name,
        default_locale,
        supported_locales,
        modules,
        extra_sites,
    })
}

pub fn sanitize_slug(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' => character,
            'A'..='Z' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn title_case_slug(input: &str) -> String {
    input
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut value = first.to_uppercase().collect::<String>();
                    value.push_str(chars.as_str());
                    value
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_slug_normalizes_names() {
        assert_eq!(sanitize_slug("My Store"), "my-store");
        assert_eq!(sanitize_slug("  coil__demo "), "coil-demo");
    }
}
