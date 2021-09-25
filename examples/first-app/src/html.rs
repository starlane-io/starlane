use starlane_resources::http::HttpResponse;
use starlane_resources::data::BinSrc;
use std::sync::Arc;
use serde_json::json;
use handlebars::Handlebars;
use handlebars::RenderError;

lazy_static! {
  pub static ref HTML: Handlebars<'static> = {
        let mut reg = Handlebars::new();
        reg.register_template_string("error-code-page", r#"

<!DOCTYPE html>
<html lang="en-US" style="background: #330000">

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
  z-index: 1;
}

#plate {
  background: radial-gradient(ellipse at bottom, #35271b 0%, #0f0a09 100%);
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  right: 0;
  z-index: -1;
}


</style>


</head>
<body>

<div id="plate"></div>

<section>
<span id="title">{{ title }}</span>
<span id="message">{{ message }}</span>
</section>



</body>
</html>






  "#);

  reg.register_template_string("mechtron-page", r#"<!DOCTYPE html>
<html lang="en" >
<head>
  <meta charset="UTF-8">
  <title>MECHTRON</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Economica:ital@1&family=Electrolize&family=Gemunu+Libre:wght@200;400&family=Geo:ital@1&family=Jura:wght@500&family=Keania+One&family=Open+Sans:ital,wght@1,300&family=Share:ital@1&family=Stalinist+One&family=Zen+Dots&display=swap" rel="stylesheet">

<style>
html{
  height: 100%;
  background-color: black;
}

body{
  background: radial-gradient(ellipse at bottom, #990000 0%, #660000 100%);
  height: 100%;
  margin: 0;
}


#title > h1 {
  text-align: center;
  font-family: 'Economica', sans-serif;
font-family: 'Stalinist One', cursive;
  margin-left: auto;
  margin-right: auto;
  margin-top: 0;
  margin-bottom: 0;
  font-size: 85px;

}

#title > h2 {
  text-align: center;
font-family: 'Zen Dots', cursive;
  margin-left: auto;
  margin-right: auto;
  margin-top: 0;
  margin-bottom: 0;

  font-size: 32px;

}

footer {

}

p {
  text-align: center;
  font-family: 'Economica', sans-serif;
font-family: 'Electrolize', sans-serif;
  margin-left: auto;
  margin-right: auto;
  margin-top: 0;
  margin-bottom: 0;

  font-size: 24px;

}


#atom {
  transform: scale(1.2, 1.2);
  overflow: hidden;
  display: block;
  width: 256px;
  height: 256px;
  border: 1px;
  margin-left: auto;
  margin-right: auto;
}

#atom > div {
  border-radius: 50%;
  border: 1px solid #000;
  transform-style: preserve-3d;
  transform: rotateX(80deg) rotateY(20deg);
  position: absolute;
  left: 50%;
  top: 50%;
  margin-left: -100px;
  margin-top: -100px;
}
#atom > div:first-of-type:after {
  content: "";
  position: absolute;
  height: 40px;
  width: 40px;
  background: #000;
  border-radius: 50%;
  transform: rotateX(-80deg) rotateY(0);
  box-shadow: 0 0 25px #000;
  -webkit-animation: nucleus_ 2s infinite linear;
          animation: nucleus_ 2s infinite linear;
  left: 50%;
  top: 50%;
  margin-top: -20px;
  margin-left: -20px;
}
#atom > div:nth-of-type(2) {
  transform: rotateX(-80deg) rotateY(20deg);
}
#atom > div:nth-of-type(2) > div,
#atom > div:nth-of-type(2) > div:after {
  -webkit-animation-delay: -0.5s;
          animation-delay: -0.5s;
}
#atom > div:nth-of-type(3) {
  transform: rotateX(-70deg) rotateY(60deg);
}
#atom > div:nth-of-type(3) > div,
#atom > div:nth-of-type(3) > div:after {
  -webkit-animation-delay: -1s;
          animation-delay: -1s;
}
#atom > div:nth-of-type(4) {
  transform: rotateX(70deg) rotateY(60deg);
}
#atom > div:nth-of-type(4) > div,
#atom > div:nth-of-type(4) > div:after {
  -webkit-animation-delay: -1.5s;
          animation-delay: -1.5s;
}
#atom > div > div {
  width: 200px;
  height: 200px;
  position: relative;
  transform-style: preserve-3d;
  -webkit-animation: trail_ 2s infinite linear;
          animation: trail_ 2s infinite linear;
}
#atom > div > div:after {
  content: "";
  position: absolute;
  top: -5px;
  box-shadow: 0 0 12px #000;
  left: 50%;
  margin-left: -5px;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background-color: #000;
  -webkit-animation: particle_ 2s infinite linear;
          animation: particle_ 2s infinite linear;
}

@-webkit-keyframes trail_ {
  from {
    transform: rotateZ(0deg);
  }
  to {
    transform: rotateZ(360deg);
  }
}

@keyframes trail_ {
  from {
    transform: rotateZ(0deg);
  }
  to {
    transform: rotateZ(360deg);
  }
}
@-webkit-keyframes particle_ {
  from {
    transform: rotateX(90deg) rotateY(0deg);
  }
  to {
    transform: rotateX(90deg) rotateY(-360deg);
  }
}
@keyframes particle_ {
  from {
    transform: rotateX(90deg) rotateY(0deg);
  }
  to {
    transform: rotateX(90deg) rotateY(-360deg);
  }
}
@-webkit-keyframes nucleus_ {
  0%, 100% {
    box-shadow: 0 0 0 transparent;
  }
  50% {
    box-shadow: 0 0 25px #000;
  }
}
@keyframes nucleus_ {
  0%, 100% {
    box-shadow: 0 0 0 transparent;
  }
  50% {
    box-shadow: 0 0 25px #000;
  }
}
</style>

</head>
<body>
<!-- partial:index.partial.html -->
<section id="title">
  <h1>MECHTRON</h1>
  <h2>Run WebAssembly Everywhere</h2>
</section>

<section id="atom">
<div>
  <div></div>
</div>
<div>
  <div></div>
</div>
<div>
  <div></div>
</div>
<div>
  <div></div>
</div>
</section>

<section>
<p>This page was served by a Mechtron</p>
</section>

</body>
</html>
            "# );

    reg
};
    }

pub fn mechtron_page( ) -> Result<HttpResponse,Error> {
    let mut response = HttpResponse::new();
    response.status = 200;
    let json = json!({});
    response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("mechtron-page", &json)?.as_bytes().to_vec())));
    Ok(response)
}

pub fn html_error_code( code: usize, title: String, message: String ) -> Result<HttpResponse,Error> {
    let mut response = HttpResponse::new();
    response.status = code;
    let json = json!({"title": title, "message": message });
    response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &json)?.as_bytes().to_vec())));
    Ok(response)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Error {
    pub error: String,
}

impl From<RenderError> for Error {
    fn from(e: RenderError) -> Self {
        Self {
            error: e.to_string()
        }
    }
}