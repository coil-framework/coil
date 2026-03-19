use std::collections::BTreeMap;

use super::RouteUrlError;

pub(super) fn match_route_path(pattern: &str, actual: &str) -> Option<BTreeMap<String, String>> {
    let pattern_segments = path_segments(pattern);
    let actual_segments = path_segments(actual);
    if pattern_segments.len() != actual_segments.len() {
        return None;
    }

    let mut params = BTreeMap::new();
    for (pattern_segment, actual_segment) in pattern_segments.iter().zip(actual_segments.iter()) {
        if pattern_segment.starts_with('{')
            && pattern_segment.ends_with('}')
            && pattern_segment.len() > 2
        {
            params.insert(
                pattern_segment[1..pattern_segment.len() - 1].to_string(),
                (*actual_segment).to_string(),
            );
        } else if pattern_segment != actual_segment {
            return None;
        }
    }

    Some(params)
}

pub(super) fn render_route_path(
    pattern: &str,
    params: &BTreeMap<String, String>,
    route_name: &str,
) -> Result<String, RouteUrlError> {
    let rendered_segments = path_segments(pattern)
        .into_iter()
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2 {
                let parameter = &segment[1..segment.len() - 1];
                params
                    .get(parameter)
                    .cloned()
                    .ok_or_else(|| RouteUrlError::MissingRouteParameter {
                        route: route_name.to_string(),
                        parameter: parameter.to_string(),
                    })
            } else {
                Ok(segment.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if rendered_segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", rendered_segments.join("/")))
    }
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}
