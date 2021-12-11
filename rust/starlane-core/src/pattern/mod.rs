

// space.org:app  // exact match of app
// space.org:app:*  // all children of 'app'

// space.org:app<App> // exact address with Type requirement
// space.org:app:db<Database<Relative>> // exact address with Type & Kind requirement .. will match to ANY specific
// space.org:app:db<Database<Relative<mysql.org:mysql:innodb:+7.0.1>>> // with specific version at 7.0.1 or up...
// space.org:app:*<*<*<mysql.org:*:*:*>>> // Any specific with mysql.org as domain

// space.org:app:*<Mechtron> // all children of 'app' that are Mechtrons
// space.org:app:** // recursive children of 'app'
// space.org:app:**<Mechtron> // recursive children of 'app' that are mechtrons
// space.org:app:**<Mechtron>:*<FileSystem>:** // all files under any mechtron filesystems

// match everything under tenant of each user
// space.org:users:*:tenant:**
//
// match everything under tenant of each user
// space.org:**<User>:tenant:**
//

// support for registry:
// space.org:app:*+blah  // all children of 'app' with a 'blah' label
// space.org:app:*+key=value // all children of 'app' with a 'key' label set to 'value'
// match everything under tenant of each user that does NOT have an admin label
// space.org:**<User>!admin:tenant:**
// space.org:[app]:**<User>:tenant:**

// Call pattern
// space.org:app:**<User>:tenant:**^Msg!*
// space.org:app:**<User>:tenant:**^Http
// space.org:app:**<User>:tenant:**^Rc

/////////////////////
// allow switch agent to pattern... and grant permissions 'crwx'
// -> { -| $admins:** +c*wx |-> $app:**<Mechtron>*; }
// allow agent pattern and permissions for sending anything to the admin/** port call
// -> { -| $admins:** +CrWX |-> $app:**<Mechtron>^Msg!admin/**; }

// -> { +( sa .:users:*||.:old-users:*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; }
// -> { +( sa .:(users|old-users):*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; }
// -> { +( sa .:(users|old-users):*; )+( grant .:my-files:** +CRUDLX; )-> $app:**<Mechtron>^Http/admins/*; }

// Http<Post>:/some/path/(.*) +( set req.path="/new/path/$1" )-[ Map{ body<Bin~json> } ]+( session )-[ Map{ headers<Meta>, body<Bin~json>, session<Text> } ]-> {*} => &;

// Msg<Action>:/work/it -> { +( sa .:users:*||.:old-users:*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; } =[ Text ]=> &;

// <App> 'taint'
// block -| $app:..:** +crwx |-| !$app:..:** +---- |
