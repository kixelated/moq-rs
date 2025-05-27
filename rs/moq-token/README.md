# moq-token

A simple JWT-based authentication scheme for moq-relay.

## Usage
```bash
moq-token generate --sign sign.key --verify verify.key
moq-token sign --key sign.key --path demo/ --publish bbb > token.jwt
moq-token verify --key verify.key < token.jwt
```