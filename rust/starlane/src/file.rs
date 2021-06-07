use std::collections::HashMap;
use std::sync::Arc;

use dyn_clone::DynClone;

use crate::error::Error;
use crate::resource::Path;
use std::fs::{File, DirBuilder};
use std::io::{Read, Write};
use std::convert::TryFrom;
use std::path::PathBuf;

pub trait FileAccess: Send+Sync+DynClone {
    fn read( &self, path: &Path )->Result<Arc<Vec<u8>>,Error>;
    fn write( &mut self, path: &Path, data: Arc<Vec<u8>> )->Result<(),Error>;
    fn with_path(&self, ext_path: String ) -> Result<Box<dyn FileAccess>,Error>;
    fn mkdir( &mut self, path: &Path ) -> Result<Box<dyn FileAccess>,Error>;
}

#[derive(Clone)]
pub struct MemoryFileAccess {
    map: HashMap<Path,Arc<Vec<u8>>>
}

impl MemoryFileAccess {
    pub fn new( ) -> Self{
        MemoryFileAccess{
            map: HashMap::new()
        }
    }
}


impl FileAccess for MemoryFileAccess {
    fn read(&self, path: &Path) -> Result<Arc<Vec<u8>>, Error> {
        self.map.get(path).cloned().ok_or(format!("could not find file for path '{}'",path.to_string()).into())
    }

    fn write(&mut self, path: &Path, data: Arc<Vec<u8>>) -> Result<(), Error> {
        self.map.insert( path.clone(), data );
        Ok(())
    }

    fn with_path(&self, base: String ) -> Result<Box<dyn FileAccess>,Error> {
        Ok(Box::new(Self::new()))
    }

    fn mkdir(&mut self, path: &Path) -> Result<Box<dyn FileAccess>, Error> {
        Ok(Box::new(Self::new()))
    }
}


#[derive(Clone)]
pub struct LocalFileAccess{
    base_dir: String
}

impl LocalFileAccess {
    pub fn new( base_dir: String) -> Self{
        LocalFileAccess{
            base_dir: base_dir
        }
    }

    pub fn cat_path(&self, path: &Path ) -> Result<String,Error> {
        if path.to_string().len() < 1 {
            return Err("path cannot be empty".into());
        }

        let mut path_str = path.to_string();
        path_str.remove(0);
        let mut path_buf = PathBuf::new();
        path_buf.push(self.base_dir.clone() );
        path_buf.push(path_str);
        let path = path_buf.as_path().clone();
        let path = path.to_str().ok_or("path error")?.to_string();
        Ok(path)
    }
}

impl FileAccess for LocalFileAccess {

    fn read(&self, path: &Path) -> Result<Arc<Vec<u8>>, Error> {
        let path = self.cat_path(path)?;

        let mut buf = vec![];
        let mut file = File::open(&path)?;
        file.read_to_end(&mut buf)?;
        Ok(Arc::new(buf))
    }

    fn write(&mut self, path: &Path, data: Arc<Vec<u8>>) -> Result<(), Error> {
        if let Option::Some(parent) = path.parent(){
            self.mkdir(&parent)?;
        }

        let path = self.cat_path(path)?;
        let mut file = File::open(&path)?;
        file.write_all(data.as_slice())?;
        Ok(())
    }

    fn with_path(&self, ext_path: String ) -> Result<Box<dyn FileAccess>, Error> {
        let path = Path::new(ext_path.as_str() )?;
        let path = self.cat_path(&path)?;
        Ok(Box::new(Self::new( path)  ))
    }

    fn mkdir(&mut self, path: &Path) -> Result<Box<dyn FileAccess>, Error> {
        let path = self.cat_path(path)?;
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(path.clone() )?;
        Ok(Box::new(Self::new( path )  ))
    }
}