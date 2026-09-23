use patternfly_yew::prelude::PageSection;
use yew::prelude::*;

/// Placeholder for the `/` dashboard route.
#[function_component(Dashboard)]
pub fn dashboard() -> Html {
    html! {
        <PageSection>
            <p>{ "TODO" }</p>
        </PageSection>
    }
}
