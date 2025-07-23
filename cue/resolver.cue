package cuenv

#Resolver: {
	ref:      string
	resolver: #ExecResolver
}

#ExecResolver: {
	command: string
	args: [...string]
}
