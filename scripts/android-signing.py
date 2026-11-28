"""Inject a release signing config into the generated Tauri Android gradle file.

Tauri regenerates src-tauri/gen/android on every android init, so CI patches the generated
file instead of committing it. Secrets arrive through the environment so no key material is
written to the repository.
"""
import sys
from pathlib import Path

SIGNING = '''    signingConfigs {
        create("release") {
            storeFile = file(System.getenv("HGHUB_KEYSTORE") ?: "release.keystore")
            storePassword = System.getenv("HGHUB_STORE_PASSWORD")
            keyAlias = System.getenv("HGHUB_KEY_ALIAS")
            keyPassword = System.getenv("HGHUB_KEY_PASSWORD")
        }
    }
'''


def main() -> int:
    path = Path(sys.argv[1])
    text = path.read_text(encoding="utf-8")
    if "signingConfigs" in text:
        print("signing config already present")
        return 0
    if "    buildTypes {" not in text or '        getByName("release") {' not in text:
        print("unexpected gradle layout; refusing to patch", file=sys.stderr)
        return 1
    text = text.replace("    buildTypes {", SIGNING + "    buildTypes {", 1)
    text = text.replace(
        '        getByName("release") {',
        '        getByName("release") {\n            signingConfig = signingConfigs.getByName("release")',
        1,
    )
    path.write_text(text, encoding="utf-8")
    print("injected release signing config")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
