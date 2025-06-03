# moq-token

A simple JWT-based authentication scheme for moq-relay.

## Usage
```bash
moq-token --key key.jwk generate
moq-token --key key.jwk sign --path demo/ --publish bbb > token.jwt
moq-token --key key.jwk verify < token.jwt
```

## Public Keys
We currently don't support public key cryptography, but we should in the future.
Patches welcome!