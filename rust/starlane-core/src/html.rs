use starlane_resources::http::HttpResponse;
use starlane_resources::data::BinSrc;
use std::sync::Arc;
use serde_json::json;
use handlebars::Handlebars;
use crate::error::Error;

lazy_static! {
  pub static ref HTML: Handlebars<'static> = {
        let mut reg = Handlebars::new();
        reg.register_template_string("error-code-page", r#"

<!DOCTYPE html>
<html lang="en-US" style="background: black">

<head>
<meta charset="utf-8">
<title>STARLANE</title>

<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Josefin+Sans:ital,wght@1,300&family=Jura&family=Stick+No+Bills:wght@200&display=swap" rel="stylesheet">
<link href="//cdn-images.mailchimp.com/embedcode/horizontal-slim-10_7.css" rel="stylesheet" type="text/css">

<style>



section{
  position: fixed;
  text-align: center;
  font-family: "jura", sans-serif;
  font-family: "Stick No Bills", sans-serif;
  font-family: "Josefin Sans", sans-serif;

  left: 50%;
  top: 50%;
  transform: translate(-50%,-50%);


}
#title{
  display: block;
  font-weight: 300;
  font-size: 128px;
  text-align: center;

  font-family: "Josefin Sans", sans-serif;
  background: -webkit-linear-gradient(white, #38495a);
  background: -webkit-linear-gradient(white, #eeaa5a);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  letter-spacing: 5px;
}

#message{
  font-weight: 200;
  font-size: 32px;

  font-family: "Josefin Sans", sans-serif;
  background: -webkit-linear-gradient(white, #38495a);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  letter-spacing: 2px;
}


</style>


</head>
<body>

<section>
<span id="title">{{ title }}</span>
<span id="message">{{ message }}</span>
</section>



</body>
</html>






  "#);
        reg
    };

}


pub fn html_error_code( code: usize, title: String, message: String ) -> Result<HttpResponse,Error> {
    let mut response = HttpResponse::new();
    response.status = code;
    let json = json!({"title": title, "message": message });
    response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &json)?.as_bytes().to_vec())));
    Ok(response)
}