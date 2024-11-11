# STARLANE FILESTORE CLI EXTERNAL SERVICE!

this is a very simple program that manages the local Os filesystem.

## GETTING STARTED

### EVNVIRONMENT VARIABLES
This program needs the environment varaible *FILE_STORE_DIR* to be set.  you can do so:

the local directory `./tmp` is a good choice since its already flagged in our .gitignore.

```
export FILE_STORE_DIR="./tmp"
```

### INIT

now we need to initialize the service: (this must be done before any other commands can be run)

```
cargo run -- init
```

### CREATE A DIRECTORY

Next you can create a directory:

```
cargo run -- mkdir subdir
```

(it can't yet automatically create parent directories a la `mkdir -p somedir/someOtherDir`

### WRITE A FILE

```
echo "Hello, this is my file" | cargo run -- write subdir/somefile.txt
```

Notice that stdin is used to transfer the content of the file

### READ A FILE

```
cargo run -- read subdir/somefile.txt

>Hello, this is my file
```

Again, stdout is used to return the contents of the file

### LIST
see the contents of a directory:

```
cargo run -- list subdir
>somefile.txt


### DELETE

```
cargo run -- delete subdir/somefile.txt 
```








