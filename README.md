# starlane
Starlane is the world's first Micro Resource Mesh.  

You may have already heard of a Service Mesh which helps your micro services find and communicate to each other in a secure and centrally configurable manner among many other things.  

A Resource Mesh helps your applications communicate directly to Resources.  Examples of micro resources inlcude: Files, Users, Database Tables, Message Queues, Schedulers, Oauth Providers, Artifacts, Configurations, Credentials and more! 

## WHAT'S THE ADVANTANGE?
In a pure service oriented archetecture the application is making requests to services to handle resources on it's behalf.  

A Resource Mesh provides a facade that allows the application to handle resources directly.  

Underneath the resource mesh is still utilizing the services however a great deal of complexity involved in locating, creating, moving, sharding and manipulating the resources has now been moved into the resource mesh and out of the application.  Less complex applications means faster development, easier to understand code and fewer bugs.

## EXAMPLE
Let's give a simple example.  

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


