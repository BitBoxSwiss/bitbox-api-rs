# Demo WebApp project

A live deployment of this demo can be found here: https://digitalbitbox.github.io/bitbox-api-rs/.

This folder contains a React project showcasing the TypeScript API. It has the bitbox-api library in
`../pkg` as a dependency. See how to build this dependency locally at the [main
README](../.github/README.md).

The main entry point of the sandbox is at [./src/App.tsx](./src/App.tsx).

The full package API is described by the TypeScript definitions file `../pkg/bitbox_api.d.ts`.

Install the deps using:

    npm i

Run the sandbox using:

    npm run dev

Hot-reloading is supported - you can recompile the WASM or change the sandbox files without
restarting the server.

To build the sandbox:

    npm run build
