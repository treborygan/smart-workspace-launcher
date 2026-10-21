# Negative security fixtures

bad_layout.json places a card outside the 12-column grid and must fail domain validation. invalid_schemes.json has disallowed javascript: URL; the schema may permit syntax but launch-domain policy must reject it. bad_archive.zip contains traversal path ../outside.json and must be rejected.
