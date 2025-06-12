# STARLANE

Starlane aims to reduce the drudgery of creating infrastructure code and shift developer focus to code that adds value
to their users. [https://starlane.io](https://starlane.io)

This packaged manages `HyperSpace` which provides infrastructure for [starlane-space](../starlane-space)

Apis (WebAssembly & external programs meant to provide custom behaviors in Starlane),

This package references the [starlane-space](../starlane-space) package and reuses of it to run the infrastructure and
it also contains mechanisms (Drivers) for extending the Starlane Type system

## WebAssembly And The Enterprise

Starlane provides a common interface abstraction so WebAssembly components can interact with any resource in the
enterprise without requiring special coding, protocols or libraries to do so. The abstraction pattern was inspired (
copied?) from the brilliant Unix File abstraction. Starlane aims to allow users to compose unrealated WebAssembly
components to make things happen that the developers never dreamed of in the same way that system admins compose
different command line utilities on Unix.

## WebAssembly what?

If you haven't heard about WebAssembly--or Wasm for short--it's basically a new virtual machine definition that can be
targed for compilation by nearly every language and--unless someone does something very stupid--Wasm executes in such a
secure sandbox that it makes Java developers blush. Wasm binaries are compact and executable at near native speeds
almost everywhere!  WebAssembly will be as revolutionary as Docker containers where ten years ago... at least that is
what I'm counting on or else I've wasted my time on this project...

## Deploy Starlane in Your Production Environment Today!

**WAIT! DON'T DO IT!**  Starlane is still a work in progress and not ready for anything approaching a production
environment. PLEASE! PLEASE! DON'T!

A demonstrable version that folks can play with should be available in a week or two (Some poor developer yanked an
essential crate that Starlane relied on and I have some rewriting to do!

## RUN LOCALLY

To install Starlane for local development simply run:

```bash
cargo install starlane
```

And to create a starlane instance:

```bash
starlane install
```

You will be led through an installation script which will install postres locally as a starlane registry.

After Starlane is installed you can run:

```bash
starlane run 
```

To connect to the running instance execute:

```bash
starlane term
```

## READ ON

If you want to learn more I recently published
a [Medium article explaining the rationale behind Starlane](https://medium.com/@uberscott/starlane-reduce-the-drudgery-of-infrastructure-code-with-webassembly-398d1b0d19f1).

## HATERS WELCOME

The proprietor of the Starlane project welcomes scathing commnents and lively discussion about Starlane... even if it
hurts our feelings. How are we supposed to make this thing better if all people do is tell us how wonderful we are?

## LINKS

The Starlane Website: [https://starlane.io](https://starlane.io)

And [The Presently Defunct Documentation](https://starlane.io/docs/) ... none of the guides work but it still paints a
picture of how Starlane is envisioned to be utilized one day.

If you want to learn more about Uberscott (that's me) I have
a [nice portfolio webpage at https://uberscott.com](https://uberscott.com) ... There's even a picture of me!`


