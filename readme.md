# remcmp
remcmp stands for **Rem**ote **C**o**mp**ilation.

I made this because I needed something that lets me compile a program on another (more beefy) machine
without setting up a whole dev environment. Only a compiler needed.

It is a small server (as well as a client that communicates with the server) that waits for build requests (more specifically,
it accepts patches/git diffs), compiles a program with those patches applied and sends the compiled binary back to the client.

Both the server and the client need to be in the same folder and on the same branch.

## Server
The server (located in `remcmp_server/`) should run on the machine that should execute the build requests.
There are some environment variables that need to be set:

- `REMCMP_AUTH` (optional): Authentication token that the client needs to provide. You should provide one if you don't
trust your network.
- `REMCMP_COMPILE_CMD` (optional, defaults to `cargo b`): Command that is run to compile the requested patch.
- `REMCMP_OUTPUT_BIN` (required): Path to the binary that is produced by the compilation. This will be sent back to the client
and is expected to be the compiled binary. If your compile command is `cargo b`, then this should be `target/debug/<binary name>`.
- `REMCMP_ADDR` (required): What socket address to bind the server on. This is usually something like `127.0.0.1:7777`

Example for running the server:
`REMCMP_OUTPUT_BIN=target/debug/<binary name> REMCMP_ADDR=127.0.0.1:7777 ~/remcmp/target/release/remcmp_server`

## Client
The client (located in `remcmp_client/`) is the program that sends the build requests to the server. When run, it executes `git diff`,
connects to the server (as specified by an environment variable) and sends the diff over. The server will then apply and compile the patch
and respond with the compiled binary. Finally, the client writes the compiled binary to a path.
Again, some environment variables need to be set:

- `REMCMP_AUTH` (optional): Authentication token. Leave empty if the server didn't specify one.
- `REMCMP_HOST` (required): Address of the host to connect to.
- `REMCMP_OUTPUT_BIN` (required): Destination for the compiled binary. This is where the binary will be written to.

Example for executing a build request by running the client and also executing the compiled binary:
`REMCMP_OUTPUT_BIN=./out REMCMP_HOST=127.0.0.1:7777 ~/remcmp/target/release/remcmp_client && ./out`
