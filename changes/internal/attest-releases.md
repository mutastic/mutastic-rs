- **Every release asset is attested, and checked against its attestation once it's out.** The
  release workflow attests each archive, `SHA256SUMS`, Scoop's `mutastic.json` and the SBOM as
  built by `release.yml` at the release's tag, and the SBOM as what each archive is made of,
  before the draft is made; `release-verify.yml` runs `gh attestation verify` on every asset it
  downloads. RELEASING.md's *Checking a download* says how to check one.
