package schema

#NixFlake: #ExecHook & {
	command: "nix"
	args: [ "print-dev-env" ]
	source: true
	inputs: ["flake.nix", "flake.lock"]
}

#Devenv: #ExecHook & {
	command: "devenv"
	args: [ "print-dev-env" ]
	source: true
	inputs: ["devenv.nix", "devenv.lock", "devenv.yaml"]
}

