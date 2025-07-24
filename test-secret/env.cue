package env

env: {
    NORMAL_VAR: "plain-value"
    SECRET_VAR: "cuenv-resolver://echo/resolved-secret"
}