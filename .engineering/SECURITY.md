# Security

Baseline risk is STANDARD; security-sensitive, signing, privileged authentication, money or irreversible operations escalate to HIGH_ASSURANCE.

Never commit secrets. External integrations use least privilege. Inputs crossing trust boundaries require validation. Dependencies and CI actions must be pinned/controlled. Web3 signing keys, production credentials and destructive deployment capabilities are never stored in HIVE context or repository plaintext.

Threat modeling and stronger proof obligations become mandatory before security-critical implementation.
