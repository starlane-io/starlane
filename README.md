# STARLANE
Starlane is the world's first **RESOURCE MESH**. 

A large amount of the complexity of your enterprise can be transferred from the application level to Starlane.  Less complexity means: faster development, easier to understand and fewer bugs.

But first...

## WHAT IS A RESOURCE MESH?

We'll explain what a Resource Mesh is shortly. First it's important for you to know that: 

An enterprise is nothing more than **Services**, **Resources** and the **Mesh** that binds them all togheter.  

For anyone who doesn't know: Resources are nouns, they are 'things', Services are 'verbs' they act upon Resources and Meshes are the universe... the medium through which all interactions take place. A primitive mesh would be your local area network and The largest mesh  would be the internet. 

You may have heard of Service Meshes before. Whereas the raw network was just a mindless medium of information transmition the innovation of Service Meshes was that knowledge could be enshrined between things instead of in things. For example, the credentials for the database didn't need to be known by the rest service. The trusted rest service could connect directly to the mesh which in turn provided the credintials to the database... This made things HUGELY easier to configure since multiple services used the same databases.  

Service meshes are amazing in many other ways, but at their core they moved complexity from Services--which there are many of--to a single Mesh which there is one of. As a rule complexity is easier to manage in one place.

Now let's consider a the developer creating an application composed of services. We'll write his code in plain english: He codes: "Send a save request message to the file service 'xyz' for file '123' and to save it in bucket 'ABC'." 

It's weird because as developers it's like we spend all day telling verbs what to do to nouns.  

I have always wanted to write my code like this: "save this file."  Can you see the difference?  When I'm talking directly to the resources some things are understood "(You, the bucket) save this file (the file I'm holding in my hand)."  Speaking directly in a clear context reduces what needs to be said which makes the meaning easier to understand while it also reduces what can go wrong.

Now, back to the technology: Of course, Resources aren't supposed to do anything, so how exactly are we going to talk to things that don't do anything? The answer is that a Resource Mesh is a facade that takes what the developer is saying to the resources and with its knowledge converts to instructions that the Services can act upon.  

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

The startWatch() method begings to watch the children of the main:uploads filesystem for changes. When a new file is added to uploads a notification is pushed via the starlane connection to the profile-processor service.  

The profile-processor service resizes the image and then copies the newly resized image to the S3 bucket by creating a new file.  

## VS THE TRADITONAL SERVICES BASED APPROACH


// this is only pseudo code for example's sake, don't try to run it!

@Service
public class UploadService {

  @Autowired
  private S3Bucket bucket;

  public void upload( String username, byte[] image ) {
     // create by specifying an address and providing the raw image bytes as the state
     var path = String.format("main:uploads:/%s<File>",username);
     starlane.create( path, image );   
  }

}



Some hidden advantages to this approach which are not seen in the code: each service only one external service connection configuration had to be supplied which was that of Starlane itself.  The developer didn't need to wrangle with, learn and configure as many APIs, without Starlane he would have had to ensure that the uploads service was being hosted on a deployment with a persistent disk, and configure the uploads directory to write to. For profile-processor he would have had to learn how to use an S3 API as well as configure the connection to the bucket and setup the bucket.  

Lastly without Starlane the two services would have needed some method of communicating with each other (uploads needs to tell profile-processor that there is a new image ready to be processed.)  This inter service communication traditionally would be handled through a message queue (like Kafka or RabbitMQ.) Both applications would have had to have libraries to facilitate communication with the message broker software and they would require configuration to connect to the service as well as coordination to make sure they were publishing and subscribing to the same queue.


