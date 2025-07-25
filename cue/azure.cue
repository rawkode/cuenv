package cuenv

#AzureKeyVaultRef: #Resolver & {
	vaultName: string
	secretName: string
	ref:       "azure-kv://\(vaultName)/\(secretName)"
	resolver: #ExecResolver & {
		command: "az"
		args: [
			"keyvault", "secret", "show",
			"--vault-name", vaultName,
			"--name", secretName,
			"--query", "value",
			"--output", "tsv",
		]
	}
}

#AzureKeyVaultCertRef: #Resolver & {
	vaultName: string
	certName:  string
	format:    string | *"pem"
	ref:       "azure-kv-cert://\(vaultName)/\(certName)"
	resolver: #ExecResolver & {
		command: "az"
		args: [
			"keyvault", "certificate", "download",
			"--vault-name", vaultName,
			"--name", certName,
			"--encoding", format,
			"--file", "/dev/stdout",
		]
	}
}