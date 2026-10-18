//! Small shared building blocks for the route views, written against the
//! PatternFly 6 CSS directly where `patternfly-yew` 0.8.2's components do
//! not give the accessibility the e2e contract needs (see
//! `e2e/README.md`): its `Alert` has no `role="alert"`, and its `Modal`
//! renders its title as an `h1` that its `aria-labelledby` does not point
//! at.

use patternfly_yew::prelude::*;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

// ------------------------------------------------------------- layout ---

#[derive(Clone, PartialEq, Properties)]
pub struct PageHeaderProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub description: Option<AttrValue>,
}

/// The page's single `h1`, in its own section.
#[function_component(PageHeader)]
pub fn page_header(props: &PageHeaderProps) -> Html {
    html! {
        <PageSection>
            <Title level={Level::H1}>{ props.title.clone() }</Title>
            if let Some(description) = &props.description {
                <p class="pf-v6-u-mt-sm pf-v6-u-color-200">{ description.clone() }</p>
            }
        </PageSection>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct LoadingProps {
    pub label: AttrValue,
}

/// Centered spinner (`role="progressbar"`).
#[function_component(Loading)]
pub fn loading(props: &LoadingProps) -> Html {
    html! {
        <Bullseye>
            <div class="pf-v6-u-p-lg">
                <Spinner aria_label={props.label.to_string()} />
            </div>
        </Bullseye>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct EmptyProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub children: Html,
}

/// PatternFly empty state with an `h3` title.
#[function_component(Empty)]
pub fn empty(props: &EmptyProps) -> Html {
    html! {
        <div class="pf-v6-c-empty-state pf-m-sm">
            <div class="pf-v6-c-empty-state__content">
                <div class="pf-v6-c-empty-state__header">
                    <div class="pf-v6-c-empty-state__title">
                        <h3 class="pf-v6-c-empty-state__title-text">{ props.title.clone() }</h3>
                    </div>
                </div>
                if props.children != Html::default() {
                    <div class="pf-v6-c-empty-state__body">{ props.children.clone() }</div>
                }
            </div>
        </div>
    }
}

// ------------------------------------------------------------- alerts ---

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Success,
    Info,
    Warning,
    Danger,
}

impl Notice {
    fn class(self) -> &'static str {
        match self {
            Notice::Success => "pf-m-success",
            Notice::Info => "pf-m-info",
            Notice::Warning => "pf-m-warning",
            Notice::Danger => "pf-m-danger",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Notice::Success => Icon::CheckCircle,
            Notice::Info => Icon::InfoCircle,
            Notice::Warning => Icon::ExclamationTriangle,
            Notice::Danger => Icon::ExclamationCircle,
        }
    }

    fn screen_reader(self) -> &'static str {
        match self {
            Notice::Success => "Success alert:",
            Notice::Info => "Info alert:",
            Notice::Warning => "Warning alert:",
            Notice::Danger => "Danger alert:",
        }
    }

    /// Errors interrupt (`alert`); everything else is a polite `status`.
    fn role(self) -> &'static str {
        match self {
            Notice::Danger => "alert",
            _ => "status",
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct InlineAlertProps {
    pub kind: Notice,
    pub title: AttrValue,
    #[prop_or_default]
    pub children: Html,
    #[prop_or_default]
    pub onclose: Option<Callback<()>>,
}

/// Inline PatternFly alert with a proper ARIA role.
#[function_component(InlineAlert)]
pub fn inline_alert(props: &InlineAlertProps) -> Html {
    let kind = props.kind;
    html! {
        <div class={classes!("pf-v6-c-alert", "pf-m-inline", kind.class(), "pf-v6-u-mb-md")} role={kind.role()}>
            <div class="pf-v6-c-alert__icon">{ kind.icon() }</div>
            <p class="pf-v6-c-alert__title">
                <span class="pf-v6-screen-reader">{ kind.screen_reader() }</span>
                { props.title.clone() }
            </p>
            if let Some(onclose) = &props.onclose {
                <div class="pf-v6-c-alert__action">
                    <button
                        class="pf-v6-c-button pf-m-plain"
                        type="button"
                        aria-label="Dismiss"
                        onclick={onclose.reform(|_| ())}
                    >
                        { Icon::Times }
                    </button>
                </div>
            }
            if props.children != Html::default() {
                <div class="pf-v6-c-alert__description">{ props.children.clone() }</div>
            }
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct ErrorAlertProps {
    pub title: AttrValue,
    pub message: AttrValue,
    #[prop_or_default]
    pub onclose: Option<Callback<()>>,
}

/// Danger alert (`role="alert"`) with the backend's message as body.
#[function_component(ErrorAlert)]
pub fn error_alert(props: &ErrorAlertProps) -> Html {
    html! {
        <InlineAlert kind={Notice::Danger} title={props.title.clone()} onclose={props.onclose.clone()}>
            <p>{ props.message.clone() }</p>
        </InlineAlert>
    }
}

/// Outcome of the last write on a page, shown above its content.
#[derive(Clone, Debug, PartialEq)]
pub enum Feedback {
    Success(String),
    Error { title: String, message: String },
}

impl Feedback {
    pub fn error(title: impl Into<String>, error: impl ToString) -> Self {
        Feedback::Error {
            title: title.into(),
            message: error.to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct FeedbackAlertProps {
    pub feedback: Option<Feedback>,
    pub onclose: Callback<()>,
}

#[function_component(FeedbackAlert)]
pub fn feedback_alert(props: &FeedbackAlertProps) -> Html {
    match &props.feedback {
        None => html!(),
        Some(Feedback::Success(message)) => html! {
            <InlineAlert kind={Notice::Success} title={message.clone()} onclose={props.onclose.clone()} />
        },
        Some(Feedback::Error { title, message }) => html! {
            <ErrorAlert title={title.clone()} message={message.clone()} onclose={props.onclose.clone()} />
        },
    }
}

// ------------------------------------------------------------- dialog ---

#[derive(Clone, PartialEq, Properties)]
pub struct DialogProps {
    /// Also the dialog's accessible name.
    pub title: AttrValue,
    pub onclose: Callback<()>,
    #[prop_or_default]
    pub children: Html,
    #[prop_or_default]
    pub footer: Html,
    #[prop_or_default]
    pub danger: bool,
}

/// Modal dialog: `role="dialog"`, `aria-modal`, named by its `h2` title.
/// Escape and the close button call `onclose`. Rendered into `<body>`.
#[function_component(Dialog)]
pub fn dialog(props: &DialogProps) -> Html {
    let title_id = use_memo((), |_| format!("dialog-title-{}", next_id()));
    let box_ref = use_node_ref();

    // On open: remember the opener, move focus into the dialog. On close:
    // give focus back to the opener if it is still on the page.
    {
        let box_ref = box_ref.clone();
        use_effect_with((), move |_| {
            let opener = active_element();
            if let Some(dialog) = box_ref.cast::<web_sys::Element>() {
                initial_focus(&dialog);
            }
            move || {
                if let Some(opener) = opener.filter(|o| o.is_connected()) {
                    let _ = opener.focus();
                }
            }
        });
    }

    let onkeydown = {
        let (onclose, box_ref) = (props.onclose.clone(), box_ref.clone());
        Callback::from(move |e: KeyboardEvent| match e.key().as_str() {
            "Escape" => {
                e.prevent_default();
                onclose.emit(());
            }
            "Tab" => {
                if let Some(dialog) = box_ref.cast::<web_sys::Element>() {
                    trap_tab(&dialog, &e);
                }
            }
            _ => {}
        })
    };
    let dialog = html! {
        <div class="pf-v6-c-backdrop">
            <div class="pf-v6-l-bullseye">
                <div
                    class="pf-v6-c-modal-box pf-m-md"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby={(*title_id).clone()}
                    tabindex="-1"
                    ref={box_ref}
                    {onkeydown}
                >
                    <div class="pf-v6-c-modal-box__close">
                        <button
                            class="pf-v6-c-button pf-m-plain"
                            type="button"
                            aria-label="Close dialog"
                            onclick={props.onclose.reform(|_| ())}
                        >
                            { Icon::Times }
                        </button>
                    </div>
                    <header class="pf-v6-c-modal-box__header">
                        <h2
                            class={classes!("pf-v6-c-modal-box__title", props.danger.then_some("pf-m-icon"))}
                            id={(*title_id).clone()}
                        >
                            { props.title.clone() }
                        </h2>
                    </header>
                    <div class="pf-v6-c-modal-box__body">{ props.children.clone() }</div>
                    <footer class="pf-v6-c-modal-box__footer">{ props.footer.clone() }</footer>
                </div>
            </div>
        </div>
    };
    // Rendered into <body>: inside the page's main section the backdrop
    // would sit under the sidebar's stacking context.
    match web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    {
        Some(body) => create_portal(dialog, body.into()),
        None => dialog,
    }
}

const FOCUSABLE: &str = "input:not([disabled]):not([type=hidden]), select:not([disabled]), \
    textarea:not([disabled]), button:not([disabled]), a[href], [tabindex]:not([tabindex='-1'])";
const FIELDS: &str = "input:not([disabled]):not([readonly]):not([type=hidden]), \
    select:not([disabled]), textarea:not([disabled]):not([readonly])";

fn active_element() -> Option<web_sys::HtmlElement> {
    use web_sys::wasm_bindgen::JsCast;
    web_sys::window()?
        .document()?
        .active_element()?
        .dyn_into::<web_sys::HtmlElement>()
        .ok()
}

fn elements(root: &web_sys::Element, selector: &str) -> Vec<web_sys::HtmlElement> {
    use web_sys::wasm_bindgen::JsCast;
    let Ok(list) = root.query_selector_all(selector) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

/// First editable field in the body; else the first footer button (the
/// confirm action); else the dialog itself.
fn initial_focus(dialog: &web_sys::Element) {
    use web_sys::wasm_bindgen::JsCast;
    let body_field = dialog
        .query_selector(".pf-v6-c-modal-box__body")
        .ok()
        .flatten()
        .and_then(|body| elements(&body, FIELDS).into_iter().next());
    let footer_button = || {
        dialog
            .query_selector(".pf-v6-c-modal-box__footer")
            .ok()
            .flatten()
            .and_then(|footer| {
                elements(&footer, "button:not([disabled])")
                    .into_iter()
                    .next()
            })
    };
    let target = body_field
        .or_else(footer_button)
        .or_else(|| dialog.clone().dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(target) = target {
        let _ = target.focus();
    }
}

/// Keeps Tab / Shift+Tab cycling inside the dialog.
fn trap_tab(dialog: &web_sys::Element, e: &KeyboardEvent) {
    let focusable = elements(dialog, FOCUSABLE);
    let (Some(first), Some(last)) = (focusable.first(), focusable.last()) else {
        e.prevent_default();
        return;
    };
    let active = active_element();
    let inside = active
        .as_ref()
        .is_some_and(|a| focusable.iter().any(|f| f == a));
    let target = if !inside {
        Some(first)
    } else if e.shift_key() && active.as_ref() == Some(first) {
        Some(last)
    } else if !e.shift_key() && active.as_ref() == Some(last) {
        Some(first)
    } else {
        None
    };
    if let Some(target) = target {
        e.prevent_default();
        let _ = target.focus();
    }
}

thread_local! {
    static NEXT_ID: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Process-unique number for element ids.
pub fn next_id() -> u32 {
    NEXT_ID.with(|n| {
        let id = n.get() + 1;
        n.set(id);
        id
    })
}

// -------------------------------------------------------------- forms ---

#[derive(Clone, PartialEq, Properties)]
pub struct TextFieldProps {
    pub id: AttrValue,
    pub label: AttrValue,
    pub value: AttrValue,
    pub oninput: Callback<String>,
    #[prop_or_default]
    pub error: Option<String>,
    #[prop_or_default]
    pub helper: Option<AttrValue>,
    #[prop_or_default]
    pub readonly: bool,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or(AttrValue::Static("text"))]
    pub input_type: AttrValue,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    /// Id of a `<datalist>` with suggestions.
    #[prop_or_default]
    pub list: Option<AttrValue>,
    #[prop_or_default]
    pub inputmode: Option<AttrValue>,
}

/// Form group: visible `<label for>`, input, helper or error text.
#[function_component(TextField)]
pub fn text_field(props: &TextFieldProps) -> Html {
    let oninput = props
        .oninput
        .reform(|e: InputEvent| e.target_unchecked_into::<HtmlInputElement>().value());
    let helper_id = format!("{}-helper", props.id);
    let has_helper = props.error.is_some() || props.helper.is_some();
    html! {
        <div class="pf-v6-c-form__group">
            <div class="pf-v6-c-form__group-label">
                <label class="pf-v6-c-form__label" for={props.id.clone()}>
                    <span class="pf-v6-c-form__label-text">{ props.label.clone() }</span>
                </label>
            </div>
            <div class="pf-v6-c-form__group-control">
                <span class={classes!(
                    "pf-v6-c-form-control",
                    props.error.is_some().then_some("pf-m-error"),
                    props.readonly.then_some("pf-m-readonly"),
                    props.disabled.then_some("pf-m-disabled"),
                )}>
                    <input
                        id={props.id.clone()}
                        name={props.id.clone()}
                        type={props.input_type.clone()}
                        value={props.value.clone()}
                        readonly={props.readonly}
                        disabled={props.disabled}
                        placeholder={props.placeholder.clone()}
                        list={props.list.clone()}
                        inputmode={props.inputmode.clone()}
                        aria-invalid={props.error.is_some().then_some("true")}
                        aria-describedby={has_helper.then(|| helper_id.clone())}
                        {oninput}
                    />
                </span>
                { helper(&helper_id, props.error.as_deref(), props.helper.as_deref()) }
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct TextAreaFieldProps {
    pub id: AttrValue,
    pub label: AttrValue,
    pub value: AttrValue,
    pub oninput: Callback<String>,
    #[prop_or_default]
    pub error: Option<String>,
    #[prop_or_default]
    pub helper: Option<AttrValue>,
    #[prop_or_default]
    pub readonly: bool,
    #[prop_or(4)]
    pub rows: u32,
}

/// [`TextField`] for multi-line values: visible `<label for>`, textarea,
/// helper or error text.
#[function_component(TextAreaField)]
pub fn text_area_field(props: &TextAreaFieldProps) -> Html {
    let oninput = props.oninput.reform(|e: InputEvent| {
        e.target_unchecked_into::<web_sys::HtmlTextAreaElement>()
            .value()
    });
    let helper_id = format!("{}-helper", props.id);
    let has_helper = props.error.is_some() || props.helper.is_some();
    html! {
        <div class="pf-v6-c-form__group">
            <div class="pf-v6-c-form__group-label">
                <label class="pf-v6-c-form__label" for={props.id.clone()}>
                    <span class="pf-v6-c-form__label-text">{ props.label.clone() }</span>
                </label>
            </div>
            <div class="pf-v6-c-form__group-control">
                <span class={classes!(
                    "pf-v6-c-form-control",
                    "pf-m-textarea",
                    props.error.is_some().then_some("pf-m-error"),
                    props.readonly.then_some("pf-m-readonly"),
                )}>
                    <textarea
                        id={props.id.clone()}
                        name={props.id.clone()}
                        rows={props.rows.to_string()}
                        value={props.value.clone()}
                        readonly={props.readonly}
                        spellcheck="false"
                        aria-invalid={props.error.is_some().then_some("true")}
                        aria-describedby={has_helper.then(|| helper_id.clone())}
                        {oninput}
                    />
                </span>
                { helper(&helper_id, props.error.as_deref(), props.helper.as_deref()) }
            </div>
        </div>
    }
}

fn helper(id: &str, error: Option<&str>, helper: Option<&str>) -> Html {
    let (text, class) = match (error, helper) {
        (Some(error), _) => (error, "pf-m-error"),
        (None, Some(helper)) => (helper, ""),
        (None, None) => return html!(),
    };
    html! {
        <div class="pf-v6-c-form__helper-text" aria-live="polite">
            <div class="pf-v6-c-helper-text">
                <div class={classes!("pf-v6-c-helper-text__item", class)} id={id.to_string()}>
                    <span class="pf-v6-c-helper-text__item-text">{ text.to_string() }</span>
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct SelectFieldProps {
    pub id: AttrValue,
    pub label: AttrValue,
    pub value: AttrValue,
    /// `(value, label)` pairs.
    pub options: Vec<(String, String)>,
    pub onchange: Callback<String>,
    #[prop_or_default]
    pub error: Option<String>,
    #[prop_or_default]
    pub disabled: bool,
}

/// Native `<select>` in a PatternFly form group.
#[function_component(SelectField)]
pub fn select_field(props: &SelectFieldProps) -> Html {
    let onchange = props
        .onchange
        .reform(|e: Event| e.target_unchecked_into::<HtmlSelectElement>().value());
    let helper_id = format!("{}-helper", props.id);
    html! {
        <div class="pf-v6-c-form__group">
            <div class="pf-v6-c-form__group-label">
                <label class="pf-v6-c-form__label" for={props.id.clone()}>
                    <span class="pf-v6-c-form__label-text">{ props.label.clone() }</span>
                </label>
            </div>
            <div class="pf-v6-c-form__group-control">
                { select_control(props.id.clone(), &props.value, &props.options, onchange, props.disabled, props.error.is_some()) }
                { helper(&helper_id, props.error.as_deref(), None) }
            </div>
        </div>
    }
}

/// A bare PatternFly-styled native `<select>` (callers supply the label).
pub fn select_control(
    id: AttrValue,
    value: &str,
    options: &[(String, String)],
    onchange: Callback<Event>,
    disabled: bool,
    error: bool,
) -> Html {
    html! {
        <span class={classes!("pf-v6-c-form-control", error.then_some("pf-m-error"))}>
            <select {id} {onchange} {disabled} aria-invalid={error.then_some("true")}>
                { for options.iter().map(|(v, label)| html! {
                    <option value={v.clone()} selected={v == value}>{ label.clone() }</option>
                }) }
            </select>
            <span class="pf-v6-c-form-control__utilities">
                <span class="pf-v6-c-form-control__toggle-icon">{ Icon::CaretDown }</span>
            </span>
        </span>
    }
}

// ------------------------------------------------------------- tables ---

/// A table cell holding a long value (DID, URN): the full value stays in
/// the DOM and in `title`, CSS truncates it visually.
pub fn long_cell(label: &'static str, value: &str) -> Html {
    html! {
        <td
            class="pf-v6-c-table__td pf-m-truncate"
            style="--pf-v6-c-table--cell--MinWidth: 10rem;"
            data-label={label}
            title={value.to_string()}
        >{ value.to_string() }</td>
    }
}

pub fn cell(label: &'static str, content: impl Into<Html>) -> Html {
    html! { <td class="pf-v6-c-table__td" data-label={label}>{ content.into() }</td> }
}

/// One full-width row holding an empty state.
pub fn empty_row(columns: usize, title: &'static str, body: Html) -> Html {
    html! {
        <tr class="pf-v6-c-table__tr">
            <td class="pf-v6-c-table__td" colspan={columns.to_string()}>
                <Empty title={title}>{ body }</Empty>
            </td>
        </tr>
    }
}

/// Column headers as `<th class="pf-v6-c-table__th pf-m-wrap" scope="col">`.
pub fn header_row(columns: &[&'static str]) -> Html {
    html! {
        <thead class="pf-v6-c-table__thead">
            <tr class="pf-v6-c-table__tr">
                { for columns.iter().map(|c| if c.is_empty() {
                    html! { <th class="pf-v6-c-table__th pf-m-wrap" scope="col"><span class="pf-v6-screen-reader">{ "Actions" }</span></th> }
                } else {
                    html! { <th class="pf-v6-c-table__th pf-m-wrap" scope="col">{ *c }</th> }
                }) }
            </tr>
        </thead>
    }
}

pub fn status_label(text: &str, color: Color) -> Html {
    html! { <Label label={text.to_string()} {color} compact=true /> }
}

#[derive(Clone, PartialEq, Properties)]
pub struct ConfirmDialogProps {
    pub title: AttrValue,
    pub confirm_label: AttrValue,
    pub onconfirm: Callback<()>,
    pub onclose: Callback<()>,
    #[prop_or_default]
    pub busy: bool,
    #[prop_or_default]
    pub children: Html,
}

/// Confirmation for a destructive action: danger button + Cancel.
#[function_component(ConfirmDialog)]
pub fn confirm_dialog(props: &ConfirmDialogProps) -> Html {
    let footer = html! {
        <>
            <PfButton
                variant={ButtonVariant::Danger}
                label={props.confirm_label.to_string()}
                disabled={props.busy}
                onclick={props.onconfirm.reform(|_| ())}
            />
            <PfButton
                variant={ButtonVariant::Link}
                label="Cancel"
                onclick={props.onclose.reform(|_| ())}
            />
        </>
    };
    html! {
        <Dialog title={props.title.clone()} onclose={props.onclose.clone()} {footer} danger=true>
            { props.children.clone() }
        </Dialog>
    }
}

/// Footer of a form dialog: submit button bound to `form` + Cancel.
pub fn form_footer(
    form: &'static str,
    submit: &'static str,
    busy: bool,
    onclose: &Callback<()>,
) -> Html {
    html! {
        <>
            <PfButton
                variant={ButtonVariant::Primary}
                r#type={ButtonType::Submit}
                form={form}
                label={submit}
                disabled={busy}
            />
            <PfButton variant={ButtonVariant::Link} label="Cancel" onclick={onclose.reform(|_| ())} />
        </>
    }
}

/// `onsubmit` handler that prevents the browser's form submission.
pub fn on_submit(callback: Callback<()>) -> Callback<SubmitEvent> {
    Callback::from(move |e: SubmitEvent| {
        e.prevent_default();
        callback.emit(());
    })
}

// ------------------------------------------------------------ buttons ---

#[derive(Clone, PartialEq, Properties)]
pub struct PfButtonProps {
    pub label: AttrValue,
    #[prop_or_default]
    pub variant: ButtonVariant,
    #[prop_or_default]
    pub size: ButtonSize,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub r#type: ButtonType,
    #[prop_or_default]
    pub form: Option<AttrValue>,
    #[prop_or_default]
    pub icon: Option<Icon>,
}

/// A PatternFly button that, unlike `patternfly_yew::Button`, keeps focus
/// after a click, so a dialog it opens can hand focus back to it.
#[function_component(PfButton)]
pub fn pf_button(props: &PfButtonProps) -> Html {
    let mut classes = classes!("pf-v6-c-button");
    classes.extend(props.variant.as_classes());
    props.size.extend_classes(&mut classes);
    let label = html! { <span class="pf-v6-c-button__text">{ props.label.clone() }</span> };
    html! {
        <button
            class={classes}
            type={props.r#type}
            form={props.form.clone()}
            disabled={props.disabled}
            onclick={props.onclick.clone()}
        >
            if let Some(icon) = props.icon {
                <span class="pf-v6-c-button__icon pf-m-start">{ icon }</span>
            }
            { label }
        </button>
    }
}
