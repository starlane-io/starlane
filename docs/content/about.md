# ABOUT STARLANE
Starlane is a **RESOURCE MESH** which can also execute client and server side WebAssembly.  It's still a work in progress and not ready for production.

Understanding what Starlane is and does can be a bit confusing because of the duality of its missions of Resource Mesh AND WebAssembly executor.  A little history clears it up somewhat:  The origin of the Starlane project was an attempt to create an environment for client and server side WebAssembly programs to deploy themselves, to securely access network resources, observe network resources for changes and message other WebAssembly programs.  

In the journey to enable WebAssembly it became apparent that Starlane's proposition would be useful to traditional microservices as well and that is when the second concept of the Resource Mesh became a first class feature in Starlane.  

Let's start by explaining what a Resource Mesh is:

## WHAT IS A RESOURCE MESH?
An enterprise is composed of **Services**, **Resources** and the **Mesh** that binds them all togheter.  

For anyone who doesn't know: Resources are nouns, they are 'things', Services are 'verbs' they act upon Resources and Meshes are the medium for the nouns and verbs to interact with each other. A primitive mesh would be your local area network and The largest mesh would be the internet. 

You may have heard of Service Meshes before. Whereas the raw network is just a mindless mechanism of information transmition the innovation of Service Mesh was that knowledge could be enshrined between things instead of in things. For example, the credentials for the database didn't need to be known by services. Trusted services can connect directly to the mesh which in turn provides the credentials to the database... This makes things HUGELY easier to configure since multiple services that use the same database.

Service meshes are amazing in many other ways, but at their core they moved complexity from Services--which there are many of--to a single Mesh which there is one of. As a rule complexity is easier to manage in one place. Like the Service Mesh the goal of a Resource Mesh is to move complexity from the application to the Resource Mesh.  Less complexity means faster development time, easier to understand code and fewer bugs.

Now let's consider a developer creating an application composed of services. We'll write his code in plain english: He codes: "Messaging Service: Send a save request message to the file service 'xyz' for file '123' and to save it in bucket 'ABC'." 

It's weird because as developers it's like we spend all day talking to elusive "verb processing machines"  telling them what we want them to do to the nouns on our behalf.

Service architectures feel like the old days--before object oriented programming--where we called functions, passing references to the data we wanted to modify instead of invoking an object method to modify the data directly.

And it is somewhat that object oriented encapsulation that I miss when I'm working with microservices.  I want to write my code like this: "Bucket,save this file."  Can you see the difference?  When I'm talking directly to the resource some things are understood: "(You, the bucket) save this file (the file I'm handing to you)."  Speaking directly in a clear context reduces what needs to be said which makes the meaning easier to understand while it also reduces what can go wrong.
 
Now, back to the technology: Of course, Resources aren't supposed to do anything, so how exactly are we going to talk to things that don't do anything? The answer is that a Resource Mesh is a facade that takes what the developer is saying to the resources and with its knowledge converts to instructions that the Services can act upon.

## WEB ASSEMBLY
TODO: Still working on the description of how WebAssembly works inside of Starlane. STAY TUNED!

## EXAMPLE
Say you have an application with a service that lets a user upload a profile picture to a mounted persistent store, and another service that sizes that image file correctly and copies the resized file to an S3 bucket.  We will call these services the 'upload' service and the 'profile-processor' service. 

In the starlane CLI we would create two filesystems resources:

```
starlane create "main:uploads<FileSystem<Standard>>"
starlane create "main:profiles<FileSystem<S3>>"
```

Above we have created two filesystems under the 'main' space.  We provide an address with a type of <FileSystem> and a kind associated with it, uploads is a <Standard> mounted filesystem kind and profiles is an <S3> bucket kind.  

Although Starlane itself is written in Rust, you can connect to a starlane instance API using a library.  We are going to write this example in Java Spring Boot.  

The only configuration we need for each services is a connection to Starlane and references to the various FileSystems they will be using (upload and profiles)

Here's the upload service:

```java
// this is only pseudo code for example's sake, don't try to run it

@Service
public class UploadService {

  @Autowired
  private Starlane starlane;

  // this value is overridden in configuration
  @Autowired
  private String uploadFileSystem = "main:uploads";


  public void upload( String username, byte[] image ) {
     // create by specifying an address and providing the raw image bytes as the state
     var path = String.format("%s:/%s<File>",uploadFileSystem,username);
     starlane.create( path, image );   
  }

}
```

That's it for the upload example.  Of course there are some problems with this simple example, what if the user uploads two profile pictures at once and there's a collision with the username being used to identify his file?  And It would be nice to use an InputStream for the image instead of holding it all in a byte buffer, we could work around these problems if this was a real application but for now this code example will serve us for illustration purposes.

Next let's dive into the profiler-processor service:


```java
@Service
public class  ProfileProcessorService{

  @Autowired
  private Starlane starlane;


  // this value is overridden in configuration
  @Autowired 
  private String uploadFileSystem = "main:uploads";
 
  // this value is overridden in configuration
  @Autowired
  private String profileFileSystem= "main:profiles";
 

  @PostConstruct
  public void startWatch(){

    // watch the children of the main:uploads FileSystem for changes (CREATE & DELETE)
    starlane.watch(uploadFileSystem, ResourceProperty.CHILDREN, (notification)-> {

     // we only want to respond to CREATE or UPDATE events, not DELETE
     if notifcation.change.kind == ResourcePropertyChange.CHILD.CREATE {

       // get the State data of the child that has changed
       State state = starlane.get( notifcation.change.getChild(), ResourceProperty.STATE );

       // grab the 'content' aspect of the state which holds the image content
       byte[] originalImage = state.get("content"); 
       
       // do some resizing work and produce a new image
       byte[] resizedImage = processImageSomehow(originalImage);

       // create the actual resizedFilePath which should exist on S3 bucket
       var username = someRegexToExtractUsername( notification.from );
       var resizedFilePath = String.format("%s:/%s<File>",profileFileSystem,username);

       // create the resized image on the S3 bucket
       starlane.create( resizedFilePath, resizedImage );
     }
    }); 
  }
}
```



It's not the best way to implement this solution in Spring, but to make things fit nicely into one class file we are using a @PostConstruct which will execute the startWatch() method after the ProfileProcessorService has been created.

The startWatch() method begins to watch the children of the main:uploads filesystem for changes. When a new file is added to uploads a notification is pushed via the starlane connection to the profile-processor service.  

The profile-processor service resizes the image and then copies the newly resized image to the S3 bucket by creating a new file.  


