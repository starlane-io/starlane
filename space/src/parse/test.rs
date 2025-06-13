use crate::command::direct::create::{PointSegTemplate, PointTemplate, Template};
use crate::command::Command;
use crate::config::Document;
use crate::err::PrintErr;
use crate::parse::context;
use crate::parse::model::{BlockKind, DelimitedBlockKind, NestedBlockKind, TerminatedBlockKind};
use crate::parse::util::{new_span, result, span_with_extra};
use crate::parse::{
    assignment, base_point_segment, base_seg, command_line, doc,
    expected_block_terminator_or_non_terminator, lex_block, lex_nested_block, lex_scope,
    lex_scope_selector, lex_scopes, lowercase1, mesh_eos, nested_block, next_stacked_name,
    path_regex, pipeline, pipeline_segment, pipeline_step_var, pipeline_stop_var, point_template,
    point_var, point_var_seg, pop, rec_version, root_ctx_seg, root_scope, root_scope_selector,
    route_attribute, scope_filter, scope_filters, skewer_case_chars, skewer_dot, space_chars,
    space_no_dupe_dots, space_point_kind_segment, space_point_segment, strip_comments, template,
    var_case, version, Env,
};
use crate::point::{Point, PointCtx, PointSegVar, RouteSegVar};
use crate::substance::Substance;
use crate::util;
use crate::util::{log, ToResolved};
use anyhow::Context;
use nom::bytes::complete::escaped;
use nom::character::complete::{alpha1, anychar, multispace0};
use nom::combinator::{all_consuming, peek, recognize};
use nom::multi::many0;
use nom::sequence::{delimited, tuple};
use nom_supreme::ParserExt;
use std::str::FromStr;
use std::sync::Arc;

#[test]
pub fn test_assignment() {
    let config = "+bin=some:bin:somewhere;";
    let assign = log(result(assignment(new_span(config)))).unwrap();
    assert_eq!(assign.key.as_str(), "bin");
    assert_eq!(assign.value.as_str(), "some:bin:somewhere");

    let config = "    +bin   =    some:bin:somewhere;";
    log(result(assignment(new_span(config)))).unwrap();

    let config = "    noplus =    some:bin:somewhere;";
    assert!(log(result(assignment(new_span(config)))).is_err());
    let config = "   +nothing ";
    assert!(log(result(assignment(new_span(config)))).is_err());
    let config = "   +nothing  = ";
    assert!(log(result(assignment(new_span(config)))).is_err());
}

#[test]
pub fn test_mechtron_config() {
    let config = r#"

Mechtron(version=1.0.0) {
    Wasm {
      +bin=repo:1.0.0:/wasm/blah.wasm;
      +name=my-mechtron;
    }
}

         "#;

    let doc = log(doc(config)).unwrap();

    if let Document::MechtronConfig(_) = doc {
    } else {
        assert!(false)
    }
}

#[test]
pub fn test_bad_mechtron_config() {
    let config = r#"

Mechtron(version=1.0.0) {
    Wasm
    varool
      +bin=repo:1.0.0:/wasm/blah.wasm;
      +name=my-mechtron;
    }
}

         "#;

    let doc = log(doc(config)).is_err();
}

#[test]
pub fn test_message_selector() {
    let route = util::log(route_attribute("#[route(\"[Topic<*>]::Ext<NewSession>\")]")).unwrap();
    let route = util::log(route_attribute("#[route(\"Hyp<Assign>\")]")).unwrap();

    println!("path: {}", route.path.to_string());
    //println!("filters: {}", route.filters.first().unwrap().name)
}

#[test]
pub fn test_create_command() {
    let command = util::log(result(command_line(new_span("create localhost<Space>")))).unwrap();
    let env = Env::new(Point::root());
    let command: Command = util::log(command.to_resolved(&env)).unwrap();
}

//    #[test]
pub fn test_command_line_err() {
    let command = util::log(result(command_line(new_span("create localhost<bad>")))).unwrap();
    let env = Env::new(Point::root());
    let command: Command = util::log(command.to_resolved(&env)).unwrap();
}

#[test]
pub fn test_point_hierarchy_FIXME() {
    let i = new_span(
        "GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>/star.bind<File>",
    );

    let i = new_span("repo<Repo>");
    //    println!("Blah: {}", blah.to_string());
    let answer = log(result(space_point_kind_segment(i))).unwrap();
    //    let answer = log(result(space_point_segment(i))).unwrap();
    println!("PointSeg: '{}'", answer.to_string());
}

#[test]
pub fn test_template() {
    let t = util::log(result(all_consuming(template)(new_span(
        "localhost<Space>",
    ))))
    .unwrap();
    let env = Env::new(Point::root());
    let t: Template = util::log(t.to_resolved(&env)).unwrap();

    let t = util::log(result(base_point_segment(new_span(
        "localhost:base<Space>",
    ))))
    .unwrap();

    let (space, bases): (PointSegVar, Vec<PointSegVar>) =
        util::log(result(tuple((
            point_var_seg(root_ctx_seg(space_point_segment)),
            many0(base_seg(point_var_seg(pop(base_point_segment)))),
        ))(new_span("localhost:base:nopo<Space>"))))
        .unwrap();
    println!("space: {}", space.to_string());
    for base in bases {
        println!("\tbase: {}", base.to_string());
    }
    //let t= util::log(result(all_consuming(template)(new_span("localhost:base<Space>")))).unwrap();
    //        let env = Env::new(Point::root());
    //       let t: Template = util::log(t.to_resolved(&env)).unwrap();
}

#[test]
pub fn test_point_template() {
    assert!(mesh_eos(new_span(":")).is_ok());
    assert!(mesh_eos(new_span("%")).is_ok());
    assert!(mesh_eos(new_span("x")).is_err());

    assert!(point_var(new_span("localhost:some-%")).is_ok());

    util::log(result(all_consuming(point_template)(new_span("localhost")))).unwrap();

    let template = util::log(result(point_template(new_span("localhost:other:some-%")))).unwrap();
    let template: PointTemplate = util::log(template.collapse()).unwrap();
    if let PointSegTemplate::Pattern(child) = template.child_segment_template {
        assert_eq!(child.as_str(), "some-%")
    }

    util::log(result(point_template(new_span("my-domain.com")))).unwrap();
    util::log(result(point_template(new_span("ROOT")))).unwrap();
}

//    #[test]
pub fn test_point_var() {
    util::log(result(all_consuming(point_var)(new_span(
        "[hub]::my-domain.com:${name}:base",
    ))))
    .unwrap();
    util::log(result(all_consuming(point_var)(new_span(
        "[hub]::my-domain.com:1.0.0:/dorko/x/",
    ))))
    .unwrap();
    util::log(result(all_consuming(point_var)(new_span(
        "[hub]::my-domain.com:1.0.0:/dorko/${x}/",
    ))))
    .unwrap();
    util::log(result(all_consuming(point_var)(new_span(
        "[hub]::.:1.0.0:/dorko/${x}/",
    ))))
    .unwrap();
    util::log(result(all_consuming(point_var)(new_span(
        "[hub]::..:1.0.0:/dorko/${x}/",
    ))))
    .unwrap();
    let point = util::log(result(point_var(new_span(
        "[hub]::my-domain.com:1.0.0:/dorko/${x}/file.txt",
    ))))
    .unwrap();
    if let Some(PointSegVar::Var(var)) = point.segments.get(4) {
        assert_eq!("x", var.name.as_str());
    } else {
        assert!(false);
    }

    if let Some(PointSegVar::File(file)) = point.segments.get(5) {
        assert_eq!("file.txt", file.as_str());
    } else {
        assert!(false);
    }

    let point = util::log(result(point_var(new_span(
        "${route}::my-domain.com:${name}:base",
    ))))
    .unwrap();

    // this one SHOULD fail and an appropriate error should be located at BAD
    util::log(result(point_var(new_span(
        "${route of routes}::my-domain.com:${BAD}:base",
    ))));

    if let RouteSegVar::Var(ref var) = point.route {
        assert_eq!("route", var.name.as_str());
    } else {
        assert!(false);
    }

    if let Some(PointSegVar::Space(space)) = point.segments.get(0) {
        assert_eq!("my-domain.com", space.as_str());
    } else {
        assert!(false);
    }

    if let Some(PointSegVar::Var(var)) = point.segments.get(1) {
        assert_eq!("name", var.name.as_str());
    } else {
        assert!(false);
    }

    if let Some(PointSegVar::Base(base)) = point.segments.get(2) {
        assert_eq!("base", base.as_str());
    } else {
        assert!(false);
    }

    let mut env = Env::new(Point::from_str("my-domain.com").unwrap());
    env.set_var("route", Substance::Text("[hub]".to_string()));
    env.set_var("name", Substance::Text("zophis".to_string()));
    let point: Point = point.to_resolved(&env).unwrap();
    println!("point.to_string(): {}", point.to_string());

    util::log(
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/x/",
        ))))
        .unwrap()
        .to_point(),
    );
    util::log(
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/${dorko}/x/",
        ))))
        .unwrap()
        .to_point(),
    );
    util::log(
        util::log(result(all_consuming(point_var)(new_span(
            "${not-supported}::my-domain.com:1.0.0:/${dorko}/x/",
        ))))
        .unwrap()
        .to_point(),
    );

    let point = util::log(result(point_var(new_span("${route}::${root}:base1")))).unwrap();
    let mut env = Env::new(Point::from_str("my-domain.com:blah").unwrap());
    env.set_var("route", Substance::Text("[hub]".to_string()));
    env.set_var("root", Substance::Text("..".to_string()));

    let point: PointCtx = util::log(point.to_resolved(&env)).unwrap();

    /*
            let resolver = Env::new(Point::from_str("my-domain.com:under:over").unwrap());
            let point = log(consume_point_var("../../hello") ).unwrap();
    //        let point: Point = log(point.to_resolved(&resolver)).unwrap();
      //      println!("point.to_string(): {}", point.to_string());
            let _: Result<Point, ExtErr> = log(log(result(all_consuming(point_var)(new_span(
                "${not-supported}::my-domain.com:1.0.0:/${dorko}/x/",
            ))).unwrap()
                .to_resolved(&env)));

             */
}

#[test]
pub fn test_point() {
    util::log(
        result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:name:base",
        )))
        .unwrap()
        .to_point(),
    )
    .unwrap();
    util::log(
        result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/x/",
        )))
        .unwrap()
        .to_point(),
    )
    .unwrap();
    util::log(
        result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/xyz/",
        )))
        .unwrap()
        .to_point(),
    )
    .unwrap();
}

#[test]
pub fn test_simple_point_var() {
    /*
    let point = util::log(result(point_var(new_span("localhost:base")))).unwrap();
    println!("point '{}'", point.to_string());
    let point :Point = point.collapse().unwrap();
    assert_eq!("localhost:base", point.to_string().as_str());
    let point = util::log(result(point_var(new_span("localhost:base<Kind>")))).unwrap();
    let point :Point = point.collapse().unwrap();
    assert_eq!("localhost:base", point.to_string().as_str());

    let point = util::log(result(point_var(new_span("localhost:base:3.0.0<Kind>")))).unwrap();
    let point :Point = point.collapse().unwrap();
    assert_eq!("localhost:base:3.0.0", point.to_string().as_str());
    let point = util::log(result(point_var(new_span("localhost:base:3.0.0:/some/file.txt<Kind>")))).unwrap();
    assert_eq!("localhost:base:3.0.0:/some/file.txt", point.to_string().as_str());
    let point :Point = point.collapse().unwrap();
    println!("point: '{}'",point.to_string());

    for seg in &point.segments {
        println!("\tseg: '{}'",seg.to_string());
    }
    assert_eq!("some/",point.segments.get(4).unwrap().to_string().as_str());

     */

    let point = util::log(result(point_var(new_span(
        "localhost:base:/fs/file.txt<Kind>",
    ))))
    .unwrap();
    let point: Point = point.collapse().unwrap();
    assert_eq!("localhost:base:/fs/file.txt", point.to_string().as_str());
}

#[test]
pub fn test_lex_block() {
    let esc = result(escaped(anychar, '\\', anychar)(new_span("\\}"))).unwrap();
    //println!("esc: {}", esc);
    util::log(result(all_consuming(lex_block(BlockKind::Nested(
        NestedBlockKind::Curly,
    )))(new_span("{}"))))
    .unwrap();
    util::log(result(all_consuming(lex_block(BlockKind::Nested(
        NestedBlockKind::Curly,
    )))(new_span("{x}"))))
    .unwrap();
    util::log(result(all_consuming(lex_block(BlockKind::Nested(
        NestedBlockKind::Curly,
    )))(new_span("{\\}}"))))
    .unwrap();
    util::log(result(all_consuming(lex_block(BlockKind::Delimited(
        DelimitedBlockKind::SingleQuotes,
    )))(new_span("'hello'"))))
    .unwrap();
    util::log(result(all_consuming(lex_block(BlockKind::Delimited(
        DelimitedBlockKind::SingleQuotes,
    )))(new_span("'ain\\'t it cool.unwrap()'"))))
    .unwrap();

    //assert!(log(result(all_consuming(lex_block( BlockKind::Nested(NestedBlockKind::Curly)))(create_span("{ }}")))).is_err());
}
#[test]
pub fn test_path_regex2() {
    util::log(result(path_regex(new_span("/xyz")))).unwrap();
}
#[test]
pub fn test_bind_config() {
    let bind_config_str = r#"Bind(version=1.0.0)  { Route<Http> -> { <Get> -> ((*)) => &; } }
        "#;

    util::log(doc(bind_config_str)).unwrap();
    if let Document::BindConfig(bind) = util::log(doc(bind_config_str)).unwrap() {
        assert_eq!(bind.route_scopes().len(), 1);
        let mut pipelines = bind.route_scopes();
        let pipeline_scope = pipelines.pop().unwrap();
        assert_eq!(pipeline_scope.selector.selector.name.as_str(), "Route");
        let message_scope = pipeline_scope.block.first().unwrap();
        assert_eq!(
            message_scope.selector.selector.name.to_string().as_str(),
            "Http"
        );
        let method_scope = message_scope.block.first().unwrap();
        assert_eq!(
            method_scope.selector.selector.name.to_string().as_str(),
            "Http<Get>"
        );
    } else {
        assert!(false);
    }

    let bind_config_str = r#"Bind(version=1.0.0)  {
              Route<Ext<Create>> -> localhost:app => &;
           }"#;

    if let Document::BindConfig(bind) = util::log(doc(bind_config_str)).unwrap() {
        assert_eq!(bind.route_scopes().len(), 1);
        let mut pipelines = bind.route_scopes();
        let pipeline_scope = pipelines.pop().unwrap();
        assert_eq!(pipeline_scope.selector.selector.name.as_str(), "Route");
        let message_scope = pipeline_scope.block.first().unwrap();
        assert_eq!(
            message_scope.selector.selector.name.to_string().as_str(),
            "Ext"
        );
        let action_scope = message_scope.block.first().unwrap();
        assert_eq!(
            action_scope.selector.selector.name.to_string().as_str(),
            "Ext<Create>"
        );
    } else {
        assert!(false);
    }

    let bind_config_str = r#"  Bind(version=1.0.0) {
              Route -> {
                 <*> -> {
                    <Get>/users/(.unwrap()P<user>)/.* -> localhost:users:${user} => &;
                 }
              }
           }

           "#;
    util::log(doc(bind_config_str)).unwrap();

    let bind_config_str = r#"  Bind(version=1.0.0) {
              Route -> {
                 <Http<*>>/users -> localhost:users => &;
              }
           }

           "#;
    util::log(doc(bind_config_str)).unwrap();

    /// the bind format is no longer valid
    /*
    let bind_config_str = r#"  Bind(version=1.0.0) {
              * -> { // This should fail since Route needs to be defined
                 <*> -> {
                    <Get>/users -> localhost:users => &;
                 }
              }
           }

           "#;
    assert!(util::log(doc(bind_config_str)).is_err());
     */
    let bind_config_str = r#"  Bind(version=1.0.0) {
              Route<Rc> -> {
                Create ; Bok;
                  }
           }

           "#;
    assert!(util::log(doc(bind_config_str)).is_err());
    //   assert!(log(config(bind_config_str)).is_err());
}

#[test]
pub fn test_pipeline_segment() {
    util::log(result(pipeline_segment(new_span("-> localhost")))).unwrap();
    assert!(util::log(result(pipeline_segment(new_span("->")))).is_err());
    assert!(util::log(result(pipeline_segment(new_span("localhost")))).is_err());
}

#[test]
pub fn test_pipeline_stop() {
    util::log(result(space_chars(new_span("localhost")))).unwrap();
    util::log(result(space_no_dupe_dots(new_span("localhost")))).unwrap();

    util::log(result(mesh_eos(new_span("")))).unwrap();
    util::log(result(mesh_eos(new_span(":")))).unwrap();

    util::log(result(recognize(tuple((
        context("point:space_segment_leading", peek(alpha1)),
        space_no_dupe_dots,
        space_chars,
    )))(new_span("localhost"))))
    .unwrap();
    util::log(result(space_point_segment(new_span("localhost.com")))).unwrap();

    util::log(
        result(point_var(new_span("mechtron.io:app:hello")))
            .unwrap()
            .to_point(),
    )
    .unwrap();
    util::log(result(pipeline_stop_var(new_span("localhost:app:hello")))).unwrap();
}

#[test]
pub fn test_pipeline() {
    util::log(result(pipeline(new_span("-> localhost => &")))).unwrap();
}

#[test]
pub fn test_pipeline_step() {
    util::log(result(pipeline_step_var(new_span("->")))).unwrap();
    util::log(result(pipeline_step_var(new_span("-[ Text ]->")))).unwrap();
    util::log(result(pipeline_step_var(new_span("-[ Text ]=>")))).unwrap();
    util::log(result(pipeline_step_var(new_span("=[ Text ]=>")))).unwrap();

    assert!(util::log(result(pipeline_step_var(new_span("=")))).is_err());
    assert!(util::log(result(pipeline_step_var(new_span("-[ Bin ]=")))).is_err());
    assert!(util::log(result(pipeline_step_var(new_span("[ Bin ]=>")))).is_err());
}

#[test]
pub fn test_rough_bind_config() {
    let unknown_config_kind = r#"
Unknown(version=1.0.0) # mem unknown config kind
{
    Route{
    }
}"#;
    let unsupported_bind_version = r#"
Bind(version=100.0.0) # mem unsupported version
{
    Route{
    }
}"#;
    let multiple_unknown_sub_selectors = r#"
Bind(version=1.0.0)
{
    Whatever -> { # Someone doesn't care what sub selectors he creates
    }

    Dude(filter $(value)) -> {}  # he doesn't care one bit!

}"#;

    let now_we_got_rows_to_parse = r#"
Bind(version=1.0.0)
{
    Route(auth) -> {
       Http {
          <$(method=.*)>/users/$(user=.*)/$(path=.*)-> localhost:app:users:$(user)^Http<$(method)>/$(path) => &;
          <Get>/logout -> localhost:app:mechtrons:logout-handler => &;
       }
    }

    Route -> {
       Ext<FullStop> -> localhost:apps:
       * -> localhost:app:bad-page => &;
    }


}"#;
    assert!(doc(unknown_config_kind).is_err());
    assert!(doc(unsupported_bind_version).is_err());
    assert!(doc(multiple_unknown_sub_selectors).is_err());
    assert!(doc(now_we_got_rows_to_parse).is_err());
}

#[test]
pub fn test_remove_comments() {
    let bind_str = r#"
# this is a mem of comments
Bind(version=1.0.0)->
{
  # let's see if it works a couple of spaces in.
  Route(auth)-> {  # and if it works on teh same line as something we wan to keep

  }

  # looky!  I deliberatly put an error here (space between the filter and the kazing -> )
  # My hope is that we will get a an appropriate error message WITH COMMENTS INTACT
  Route(noauth)-> # look!  I made a boo boo
  {
     # nothign to see here
  }
}"#;

    match doc(bind_str) {
        Ok(_) => {}
        Err(err) => {
            err.print();
        }
    }
}

#[test]
pub fn test_version() {
    rec_version(new_span("1.0.0")).unwrap();
    rec_version(new_span("1.0.0-alpha")).unwrap();
    version(new_span("1.0.0-alpha")).unwrap();
}
#[test]
pub fn test_rough_block() {
    result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
        new_span("{  }"),
    ))
    .unwrap();
    result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
        new_span("{ {} }"),
    ))
    .unwrap();
    assert!(
        result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
            new_span("{ } }")
        ))
        .is_err()
    );
    // this is allowed by rough_block
    result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
        new_span("{ ] }"),
    ))
    .unwrap();

    result(lex_nested_block(NestedBlockKind::Curly)(new_span(
        r#"x blah


Hello my friend


        }"#,
    )))
    .err()
    .unwrap()
    .print();

    result(lex_nested_block(NestedBlockKind::Curly)(new_span(
        r#"{

Hello my friend


        "#,
    )))
    .err()
    .unwrap()
    .print();
}

#[test]
pub fn test_block() {
    util::log(result(lex_nested_block(NestedBlockKind::Curly)(new_span(
        "{ <Get> -> localhost; }    ",
    ))))
    .unwrap();

    all_consuming(nested_block(NestedBlockKind::Curly))(new_span("{  }")).unwrap();
    all_consuming(nested_block(NestedBlockKind::Curly))(new_span("{ {} }")).unwrap();
    util::log(result(nested_block(NestedBlockKind::Curly)(new_span(
        "{ [] }",
    ))))
    .unwrap();
    assert!(
        expected_block_terminator_or_non_terminator(NestedBlockKind::Curly)(new_span("}")).is_ok()
    );
    assert!(
        expected_block_terminator_or_non_terminator(NestedBlockKind::Curly)(new_span("]")).is_err()
    );
    assert!(
        expected_block_terminator_or_non_terminator(NestedBlockKind::Square)(new_span("x")).is_ok()
    );
    assert!(nested_block(NestedBlockKind::Curly)(new_span("{ ] }")).is_err());
    result(nested_block(NestedBlockKind::Curly)(new_span(
        r#"{



        ]


        }"#,
    )))
    .err()
    .unwrap()
    .print();
}

//#[test]
pub fn test_root_scope_selector() {
    assert!(
        (result(root_scope_selector(new_span(
            r#"

            Bind(version=1.0.0)->"#,
        )))
        .is_ok())
    );

    assert!(
        (result(root_scope_selector(new_span(
            r#"

            Bind(version=1.0.0-alpha)->"#,
        )))
        .is_ok())
    );

    result(root_scope_selector(new_span(
        r#"

            Bind(version=1.0.0) ->"#,
    )))
    .err()
    .unwrap()
    .print();

    result(root_scope_selector(new_span(
        r#"

        Bind   x"#,
    )))
    .err()
    .unwrap()
    .print();

    result(root_scope_selector(new_span(
        r#"

        (Bind(version=3.2.0)   "#,
    )))
    .err()
    .unwrap()
    .print();
}

//    #[test]
pub fn test_scope_filter() {
    result(scope_filter(new_span("(auth)"))).unwrap();
    result(scope_filter(new_span("(auth )"))).unwrap();
    result(scope_filter(new_span("(auth hello)"))).unwrap();
    result(scope_filter(new_span("(auth +hello)"))).unwrap();
    result(scope_filters(new_span("(auth +hello)->"))).unwrap();
    result(scope_filters(new_span("(auth +hello)-(filter2)->"))).unwrap();
    result(scope_filters(new_span("(3auth +hello)-(filter2)->")))
        .err()
        .unwrap()
        .print();
    result(scope_filters(new_span("(a.unwrap()th +hello)-(filter2)->")))
        .err()
        .unwrap()
        .print();
    result(scope_filters(new_span("(auth +hello)-(filter2) {}")))
        .err()
        .unwrap()
        .print();

    assert!(skewer_case_chars(new_span("3x")).is_err());
}
#[test]
pub fn test_next_selector() {
    assert_eq!(
        "Http",
        next_stacked_name(new_span("Http"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );
    assert_eq!(
        "Http",
        next_stacked_name(new_span("<Http>"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );
    assert_eq!(
        "Http",
        next_stacked_name(new_span("Http<Ext>"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );
    assert_eq!(
        "Http",
        next_stacked_name(new_span("<Http<Ext>>"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );

    assert_eq!(
        "*",
        next_stacked_name(new_span("<*<Ext>>"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );

    assert_eq!(
        "*",
        next_stacked_name(new_span("*"))
            .unwrap()
            .1
             .0
            .to_string()
            .as_str()
    );

    assert!(next_stacked_name(new_span("<*x<Ext>>")).is_err());
}
#[test]
pub fn test_lex_scope2() {
    /*        let scope = log(result(lex_scopes(create_span(
               "  Get -> {}\n\nPut -> {}   ",
           )))).unwrap();

    */
    util::log(result(many0(delimited(
        multispace0,
        lex_scope,
        multispace0,
    ))(new_span(""))))
    .unwrap();
    util::log(result(path_regex(new_span("/root/$(subst)")))).unwrap();
    util::log(result(path_regex(new_span("/users/$(user=.*)")))).unwrap();
}

#[test]
pub fn test_lex_scope() {
    let pipes = util::log(result(lex_scope(new_span("Pipes -> {}")))).unwrap();

    //        let pipes = log(result(lex_scope(create_span("Pipes {}"))));

    assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.block.content.len(), 0);
    assert!(pipes.selector.filters.is_empty());
    assert!(pipes.pipeline_step.is_some());

    assert!(util::log(result(lex_scope(new_span("Pipes {}")))).is_err());

    let pipes = util::log(result(lex_scope(new_span("Pipes -> 12345;")))).unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
    assert_eq!(pipes.block.content.to_string().as_str(), "-> 12345");
    assert_eq!(
        pipes.block.kind,
        BlockKind::Terminated(TerminatedBlockKind::Semicolon)
    );
    assert_eq!(pipes.selector.filters.len(), 0);
    assert!(pipes.pipeline_step.is_none());
    let pipes = util::log(result(lex_scope(new_span(
        //This time adding a space before the 12345... there should be one space in the content, not two
        r#"Pipes ->  12345;"#,
    ))))
    .unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
    assert_eq!(pipes.block.content.to_string().as_str(), "->  12345");
    assert_eq!(
        pipes.block.kind,
        BlockKind::Terminated(TerminatedBlockKind::Semicolon)
    );
    assert_eq!(pipes.selector.filters.len(), 0);
    assert!(pipes.pipeline_step.is_none());

    let pipes = util::log(result(lex_scope(new_span("Pipes(auth) -> {}")))).unwrap();

    assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
    assert_eq!(pipes.block.content.len(), 0);
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 1);
    assert!(pipes.pipeline_step.is_some());

    let pipes = util::log(result(lex_scope(new_span("Route<Ext> -> {}")))).unwrap();

    assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
    assert_eq!(
        Some(
            pipes
                .selector
                .children
                .as_ref()
                .unwrap()
                .to_string()
                .as_str()
        ),
        Some("<Ext>")
    );

    assert_eq!(pipes.block.content.to_string().as_str(), "");
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 0);
    assert!(pipes.pipeline_step.is_some());

    let pipes = util::log(result(lex_scope(new_span(
        "Route<Http>(noauth) -> {zoink!{}}",
    ))))
    .unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
    assert_eq!(
        Some(
            pipes
                .selector
                .children
                .as_ref()
                .unwrap()
                .to_string()
                .as_str()
        ),
        Some("<Http>")
    );
    assert_eq!(pipes.block.content.to_string().as_str(), "zoink!{}");
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 1);
    //        assert_eq!(Some(pipes.pipeline_step.unwrap().to_string().as_str()),Some("->") );

    let msg = "Hello my future friend";
    let parseme = format!("<Http<Get>> -> {};", msg);
    let pipes = util::log(result(lex_scope(new_span(parseme.as_str())))).unwrap();

    assert_eq!(pipes.selector.name.to_string().as_str(), "Http");
    assert_eq!(
        pipes.block.content.to_string().as_str(),
        format!("-> {}", msg)
    );
    assert_eq!(
        pipes.block.kind,
        BlockKind::Terminated(TerminatedBlockKind::Semicolon)
    );
    assert_eq!(pipes.selector.filters.len(), 0);
    assert!(pipes.pipeline_step.is_none());

    assert_eq!(
        lex_scope_selector(new_span("<Route<Http>>/users/",))
            .unwrap()
            .0
            .len(),
        0
    );

    util::log(result(lex_scope_selector(new_span(
        "Route<Http<Get>>/users/",
    ))))
    .unwrap();

    let pipes = util::log(result(lex_scope(new_span(
        "Route<Http<Get>>/blah -[Text ]-> {}",
    ))))
    .unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
    assert_eq!(
        Some(
            pipes
                .selector
                .children
                .as_ref()
                .unwrap()
                .to_string()
                .as_str()
        ),
        Some("<Http<Get>>")
    );
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 0);
    assert_eq!(
        pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
        "-[Text ]->"
    );

    let pipes = util::log(result(lex_scope(new_span(
        "Route<Http<Get>>(auth)/users/ -[Text ]-> {}",
    ))))
    .unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
    assert_eq!(
        Some(
            pipes
                .selector
                .children
                .as_ref()
                .unwrap()
                .to_string()
                .as_str()
        ),
        Some("<Http<Get>>")
    );
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 1);
    assert_eq!(
        pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
        "-[Text ]->"
    );

    let pipes = util::log(result(lex_scope(new_span(
        "Route<Http<Get>>(auth)-(blah xyz)/users/ -[Text ]-> {}",
    ))))
    .unwrap();
    assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
    assert_eq!(
        Some(
            pipes
                .selector
                .children
                .as_ref()
                .unwrap()
                .to_string()
                .as_str()
        ),
        Some("<Http<Get>>")
    );
    assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
    assert_eq!(pipes.selector.filters.len(), 2);
    assert_eq!(
        pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
        "-[Text ]->"
    );

    let (next, stripped) = strip_comments(new_span(
        r#"Route<Http>(auth)-(blah xyz)/users/ -[Text]-> {

            Get -> {}
            <Put>(superuser) -> localhost:app => &;
            Post/users/scott -> localhost:app^Ext<SuperScott> => &;

        }"#,
    ))
    .unwrap();
    let span = span_with_extra(stripped.as_str(), Arc::new(stripped.to_string()));
    let pipes = util::log(result(lex_scope(span))).unwrap();

    let pipes = util::log(result(lex_scope(new_span("* -> {}")))).unwrap();

    /* let pipes = log(result(lex_scope(create_span(
        "* -> {}",
    )))).unwrap();

    */
}

pub fn test_nesting_bind() {
    let pipes = util::log(result(lex_scope(new_span(
        r#"


            Route<Http>/auth/.*(auth) -> {

                   <Get>/auth/more ->

            }"#,
    ))))
    .unwrap();
}

//#[test]
pub fn test_root_and_subscope_phases() {
    let config = r#"
Bind(version=1.2.3)-> {
   Route -> {
   }

   Route(auth)-> {
   }
}

        "#;

    let root = result(root_scope(new_span(config))).unwrap();

    util::log(lex_scopes(root.block.content.clone()));
    let sub_scopes = lex_scopes(root.block.content.clone()).unwrap();

    assert_eq!(sub_scopes.len(), 2);
}
#[test]
pub fn test_variable_name() {
    assert_eq!(
        "v".to_string(),
        util::log(result(lowercase1(new_span("v"))))
            .unwrap()
            .to_string()
    );
    assert_eq!(
        "var".to_string(),
        util::log(result(skewer_dot(new_span("var"))))
            .unwrap()
            .to_string()
    );

    util::log(result(var_case(new_span("var")))).unwrap();
}

//#[test]
pub fn test_subst() {
    /*
    #[derive(Clone)]
    pub struct SomeParser();
    impl SubstParser<String> for SomeParser {
        fn parse_span<'a>(&self, span: I) -> Res<I, String> {
            recognize(terminated(
                recognize(many0(pair(peek(not(eof)), recognize(anychar)))),
                eof,
            ))(span)
            .map(|(next, span)| (next, span.to_string()))
        }
    }

    let chunks = log(result(subst(SomeParser())(create_span("123[]:${var}:abc")))).unwrap();
    assert_eq!(chunks.chunks.len(), 3);
    let mut resolver = MapResolver::new();
    resolver.insert("var", "hello");
    let resolved = log(chunks.resolve_vars(&resolver)).unwrap();

    let chunks = log(result(subst(SomeParser())(create_span(
        "123[]:\\${var}:abc",
    )))).unwrap();
    let resolved = log(chunks.resolve_vars(&resolver)).unwrap();

    let r = log(result(subst(SomeParser())(create_span(
        "123[    ]:${var}:abc",
    )))).unwrap();
    println!("{}", r.to_string());
    log(result(subst(SomeParser())(create_span("123[]:${vAr}:abc"))));
    log(result(subst(SomeParser())(create_span(
        "123[]:${vAr }:abc",
    ))));



     */
    unimplemented!()
}

#[test]
pub fn space_point() {
    assert!(log(result(space_point_segment(new_span("lah.com")))).is_ok());
}
