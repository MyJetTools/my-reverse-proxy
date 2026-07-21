use dioxus::prelude::*;

use crate::models::InitSslCertificateResultModel;

/// What the dashboard asks the upload dialog to work on. `None` in the owning signal means
/// the dialog is closed.
#[derive(Clone, PartialEq)]
pub struct SslCertDialogRequest {
    /// Id of the certificate as the endpoint references it in the configuration.
    pub cert_id: String,
    /// `host` of the endpoint which is missing the certificate, e.g. `example.com:443`.
    pub endpoint_host: String,
}

/// Outcome of the last "Apply" click.
#[derive(Clone, PartialEq)]
enum ApplyState {
    Editing,
    Applying,
    Failed(String),
    Applied(InitSslCertificateResultModel),
}

#[component]
pub fn SslCertDialog(
    request: SslCertDialogRequest,
    dialog: Signal<Option<SslCertDialogRequest>>,
) -> Element {
    let mut certificate = use_signal(String::new);
    let mut private_key = use_signal(String::new);
    let mut apply_state = use_signal(|| ApplyState::Editing);

    let required_sni = required_sni(&request.endpoint_host);
    let state = apply_state.read().clone();
    let applying = state == ApplyState::Applying;

    let cert_id = request.cert_id.clone();

    let on_apply = move |_| {
        let cert_pem = certificate.read().trim().to_string();
        let key_pem = private_key.read().trim().to_string();

        if let Err(err) = validate_pem(&cert_pem, &key_pem) {
            apply_state.set(ApplyState::Failed(err));
            return;
        }

        let cert_id = cert_id.clone();
        apply_state.set(ApplyState::Applying);

        spawn(async move {
            match crate::api::init_ssl_certificate(&cert_id, &cert_pem, &key_pem).await {
                Ok(result) => apply_state.set(ApplyState::Applied(result)),
                Err(err) => apply_state.set(ApplyState::Failed(err)),
            }
        });
    };

    let title = format!("Upload SSL certificate — {}", request.cert_id);

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| dialog.set(None),
            div {
                class: "modal-panel",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "modal-header",
                    span { class: "modal-title", "{title}" }
                    button {
                        class: "modal-close",
                        onclick: move |_| dialog.set(None),
                        "✕"
                    }
                }
                div { class: "modal-body",
                    div { class: "cert-form",
                        {render_target(&request, required_sni.as_deref())}
                        if let ApplyState::Applied(result) = &state {
                            {render_success(result)}
                            div { class: "cert-actions",
                                button {
                                    class: "cert-apply-btn",
                                    onclick: move |_| dialog.set(None),
                                    "Close"
                                }
                            }
                        } else {
                            div { class: "cert-field",
                                label { class: "cert-label", "Certificate — PEM, full chain" }
                                textarea {
                                    class: "cert-input",
                                    rows: 10,
                                    // Keep browser cloud spell-check / autocomplete away from the
                                    // pasted key material.
                                    spellcheck: "false",
                                    autocomplete: "off",
                                    autocorrect: "off",
                                    placeholder: "-----BEGIN CERTIFICATE-----",
                                    value: "{certificate}",
                                    oninput: move |evt| certificate.set(evt.value()),
                                }
                            }
                            div { class: "cert-field",
                                label { class: "cert-label", "Private key — PEM" }
                                textarea {
                                    class: "cert-input",
                                    rows: 8,
                                    spellcheck: "false",
                                    autocomplete: "off",
                                    autocorrect: "off",
                                    placeholder: "-----BEGIN PRIVATE KEY-----",
                                    value: "{private_key}",
                                    oninput: move |evt| private_key.set(evt.value()),
                                }
                            }
                            if let ApplyState::Failed(err) = &state {
                                div { class: "cert-error", "{err}" }
                            }
                            div { class: "cert-actions",
                                button {
                                    class: "cert-apply-btn",
                                    disabled: applying,
                                    onclick: on_apply,
                                    if applying { "Applying..." } else { "Apply" }
                                }
                                button {
                                    class: "cert-cancel-btn",
                                    onclick: move |_| dialog.set(None),
                                    "Cancel"
                                }
                            }
                            div { class: "cert-note",
                                "The proxy checks that the private key matches the certificate and that the certificate covers the SNI of this endpoint before installing it. Nothing is stored if the check fails."
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What the certificate is going to be installed for — shown before the operator pastes
/// anything, so an upload for the wrong endpoint is caught by eye.
fn render_target(request: &SslCertDialogRequest, required_sni: Option<&str>) -> Element {
    rsx! {
        div { class: "cert-target",
            div { class: "cert-target-row",
                span { class: "cert-target-label", "Endpoint" }
                span { class: "cert-target-value", "{request.endpoint_host}" }
            }
            div { class: "cert-target-row",
                span { class: "cert-target-label", "Cert id" }
                span { class: "cert-target-value", "{request.cert_id}" }
            }
            if let Some(sni) = required_sni {
                div { class: "cert-target-row",
                    span { class: "cert-target-label", "Must cover" }
                    span { class: "cert-target-value", "{sni}" }
                }
            } else {
                div { class: "cert-warning",
                    "This endpoint has no server name configured, so there is no SNI to validate against. The proxy accepts the upload only if it can compare the certificate with one already loaded under this id."
                }
            }
        }
    }
}

fn render_success(result: &InitSslCertificateResultModel) -> Element {
    let covered_domains = result.covered_domains.join(", ");
    let validated_sni = result.validated_sni.join(", ");

    rsx! {
        div { class: "cert-success",
            div { class: "cert-success-title", "Certificate installed" }
            div { class: "cert-target-row",
                span { class: "cert-target-label", "CN" }
                span { class: "cert-target-value", "{result.cn}" }
            }
            div { class: "cert-target-row",
                span { class: "cert-target-label", "Expires" }
                span { class: "cert-target-value", "{result.expires}" }
            }
            div { class: "cert-target-row",
                span { class: "cert-target-label", "Valid for" }
                span { class: "cert-target-value", "{covered_domains}" }
            }
            div { class: "cert-target-row",
                span { class: "cert-target-label", "Verified SNI" }
                span { class: "cert-target-value", "{validated_sni}" }
            }
            div { class: "cert-note",
                "Served from the next TLS handshake — no reload needed. The certificate is now manually managed and is not auto-renewed until its configured source is refreshed."
            }
        }
    }
}

/// The SNI the uploaded certificate has to cover. `host` is `<server name>:<port>` for a named
/// endpoint; an endpoint listening without a server name (bare port, unix socket) has none.
fn required_sni(host: &str) -> Option<String> {
    let (server_name, _) = host.split_once(':')?;

    if server_name.is_empty() {
        return None;
    }

    Some(server_name.to_string())
}

/// Catches the obvious mistakes without a round trip. The authoritative validation — the key
/// matching the certificate and the certificate covering the endpoint SNI — is done by the proxy.
fn validate_pem(certificate: &str, private_key: &str) -> Result<(), String> {
    if certificate.is_empty() {
        return Err("Paste the certificate PEM first".to_string());
    }

    if private_key.is_empty() {
        return Err("Paste the private key PEM first".to_string());
    }

    if !certificate.starts_with("-----BEGIN") {
        return Err(
            "The certificate does not look like PEM — it has to start with '-----BEGIN CERTIFICATE-----'"
                .to_string(),
        );
    }

    if !private_key.starts_with("-----BEGIN") {
        return Err(
            "The private key does not look like PEM — it has to start with '-----BEGIN ... PRIVATE KEY-----'"
                .to_string(),
        );
    }

    Ok(())
}
