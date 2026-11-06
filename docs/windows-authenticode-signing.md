# Windows Authenticode signing

Tukkiiie supports Authenticode signing for Windows binaries. This is the correct distribution path for devices that enforce Smart App Control; a self-signed certificate is not suitable for public distribution.

## What gets signed

- The main application executable and the generated NSIS installer are signed by `cargo-packager` when a Windows signing thumbprint is configured.
- Each Windows plugin executable is signed and verified before it is placed in its plugin archive.
- Windows development builds remain buildable without a certificate, but the workflow warns that the resulting artifact is unsigned and may be blocked by Smart App Control.
- Production Windows app and plugin release jobs fail unless a usable code-signing certificate is provisioned.

Signatures use SHA-256 and an RFC 3161 timestamp. The default timestamp authority is `http://timestamp.digicert.com`; it can be overridden with the `BONGOCAT_WINDOWS_TIMESTAMP_URL` Actions variable.

## Certificate provisioning

Use a code-signing certificate issued by a certificate authority in Microsoft's Trusted Root Program. The certificate must include the Code Signing enhanced key usage and its private key must be usable by the Windows CI runner.

The signing preparation script supports two arrangements:

1. **PFX import** (only when the certificate provider explicitly permits a CI-usable PFX): add these GitHub Actions repository secrets:
   - `BONGOCAT_WINDOWS_SIGNING_PFX_BASE64`: the PFX file encoded as Base64.
   - `BONGOCAT_WINDOWS_SIGNING_PFX_PASSWORD`: the PFX password.
   - Optionally, `BONGOCAT_WINDOWS_SIGNING_THUMBPRINT`: expected certificate thumbprint, used as an additional check.

   The workflow imports the private key into the runner's current-user Personal certificate store without marking it exportable, then deletes the temporary PFX file.

2. **Provider-managed key**: configure the certificate provider's supported CI integration so the certificate and private key are available in the runner's current-user Personal certificate store. Set the `BONGOCAT_WINDOWS_SIGNING_THUMBPRINT` repository secret to the certificate thumbprint. The provider must make the private key usable by SignTool without exporting it.

Optional repository variables:
- `BONGOCAT_WINDOWS_SIGNTOOL`: full path to `signtool.exe`. If omitted, the preparation script locates the Windows SDK copy.
- `BONGOCAT_WINDOWS_TIMESTAMP_URL`: RFC 3161 timestamp URL. If omitted, the default above is used.

Do not commit certificates, private keys, PFX files, or passwords. Never paste them into chat, an issue, or a commit. Do not attempt to export a key from a hardware security module or other non-exportable provider; use the provider's supported CI signing integration instead.

Any account enrollment, identity validation, or billing must use accurate information and be completed by the person legally responsible for that account and certificate. Do not misrepresent identity or age to obtain a certificate.

## Workflow behavior

- `.github/workflows/tukkiiie-windows-build.yml`: signing is optional for development. With no certificate, it produces a clearly warned unsigned test build.
- `.github/workflows/app-release.yml`: trusted Authenticode signing is mandatory for Windows production releases.
- `.github/workflows/plugin-release.yml`: trusted Authenticode signing is mandatory for Windows plugin releases.

A valid signature is a key trust signal, not a promise that every Windows reputation warning disappears immediately. Microsoft explains that Smart App Control uses both its app-intelligence verdict and trusted signatures. See [Smart App Control overview](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview) and [Windows code-signing options](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
