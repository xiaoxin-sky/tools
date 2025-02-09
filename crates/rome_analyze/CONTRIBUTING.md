# Analyzer

The analyzer is a generic crate aimed to implement a visitor-like infrastructure, where
it's possible to inspect a piece of AST and emit diagnostics or actions based on a 
static check.

# Folder structure

First of all, you need to identify the crate where you want to implement the rule. If the rule
is going to be implemented for the JavaScript language (and its super languages), then the rule
will be implemented inside the `rome_js_analyzer` crate.

Rules are divided by capabilities:
- `analyzers/` folder contains rules that don't require any particular capabilities;
- `semantic_analyzer/` folder contains rules that require the use of the semantic model;
- `assists/` folder contains rules that contribute to refactor code, with not associated diagnostics;
these are rules that are usually meant for editors/IDEs;

Most of the rules will go under `semantic_analyzer/` or  `analyzers/` folders.

Inside the folders, we will have folders for each group that Rome supports.

When implementing **new rules**, they have to be implemented under the group `nursery`. New rules should
always be considered unstable/not exhaustive.

## Lint rules

This gives to the project time to test the rule, find edge cases, etc.

When creating or updating lint rules, you need to be aware that there's a lot of generated code inside our toolchain, and
our CI makes sure that this code is not out of sync. If some autogenerated files are out of sync,
the CI will fail.

### Create your first rule

Let's say we want to create a new rule called `useAwesomeTricks`, which uses the semantic model.

1. create a new file under `semantic_analyzers/nursery` called `use_awesome_tricks`;
2. run the cargo alias `cargo codegen analyzer`, this command will update the file called `nursery.rs`
inside the `semantic_analyzers` folder
3. from there, use the [`declare_rule`](#declare_rule) macro to create a new type
   ```rust,ignore
   use rome_analyze::declare_rule;
    
   declare_rule! {
     /// Promotes the use of awesome tricks
     /// 
     /// ## Examples
     ///
     /// ### Invalid
     ///
     pub(crate) UseAwesomeTricks {
         version: "0.10.0",
         name: "useAwesomeTricks",
         recommended: false,
        }
    }
   ```
4. Then you need to use the `Rule` trait to implement the rule on this new created struct
   ```rust,ignore
   use rome_analyze::{Rule, RuleCategory};
   use rome_js_syntax::JsAnyExpression;
   use rome_analyze::context::RuleContext;
   
   impl Rule for UseAwesomeTricks {
        const CATEGORY: RuleCategory = RuleCategory::Lint;
        type Query = Semantic<JsAnyExpression>;
        type State = String;
        type Signals = Option<Self::State>;
        type Options = ();
   
        fn run(ctx: &RuleContext<Self>) -> Self::Signals {}
   }
   ```
5. the const `CATEGORY` must be `RuleCategory::Lint` otherwise it won't work
6. the `Query` needs to have the `Semantic` type, because we want to have access to the semantic model.
`Query` tells the engine on which AST node we want to trigger the rule. 
7. The `State` type doesn't have to be used, so it can be considered optional, but it has
be defined as `type State = ()`
8. The `run` function must be implemented. This function is called every time the analyzer
finds a match for the query specified by the rule, and may return zero or more "signals".
9. Implement the optional `diagnostic` function, to tell the user where's the error and why:
    ```rust,ignore
    impl Rule for UseAwesomeTricks {
        // .. code
        fn diagnostic(_ctx: &RuleContext<Self>, _state: &Self::State) -> Option<RuleDiagnostic> {}
    }
    ```
    While implementing the diagnostic, please keep [Rome's technical principals](https://rome.tools/#technical) in mind.
    This function is called of every signal emitted by the `run` function, and it may return
    zero or one diagnostic. 

    You will have to manually update the file `rome_diagnostics_categories/src/categories.rs` and add a new category
    for the new rule you're about to create.
10. Implement the optional `action` function, if we are able to provide automatic code fix to the rule:
    ```rust,ignore
    impl Rule for UseAwesomeTricks {
        // .. code
        fn action(_ctx: &RuleContext<Self>, _state: &Self::State) -> Option<JsRuleAction> {}
    }
    ```
    This function is called of every signal emitted by the `run` function, and it may return
    zero or one code action.
    When returning a code action, you will need to pass `category` and `applicability` fields.
    `category` must be `ActionCategory::QuickFix`, while `applicability` must be `Applicability:MaybeIncorrect`

That's it! Now, let's test the rule

### Test a rule

Inside the `tests/spec` folder, rules are divided by group and rule name. The test infrastructure
is rigid around the association of the pair "group/rule name", which means that _**your test cases
are placed inside the wrong group, you won't see any diagnostics**_.

Since each new rule will start from `nursery`, that's where we start.

From there, you have two options:
- create a single file called like the rule name, e.g. `useAwesomeTricks.js`, `useAwesomeTricks.tsx`, etc. The
extension of the file matters based on which kind of file you need to test;
- create a folder called `useAwesomeTricks/`, and then create various files where you want to create
different cases. These options are useful if your rules target different super languages, or you want
to split your cases among different files;

Run the command `cargo test` and if you've done everything correctly, you should see some snapshots
emitted with diagnostics and code actions.

Check our main [contribution document](https://github.com/rome/tools/blob/main/CONTRIBUTING.md#snapshot-tests)
to know how to deal with the snapshot tests.

### Code generation

Run the following commands to update the generated files:

- `cargo codegen-configuration`, **this command must be run first** and, it will update the configuration;
- `cargo lintdoc`, it will update the website with the documentation of the rules, check [`declare_rule`](#declare_rule)
for more information about it;
- `cargo codegen-bindings`, it will update the TypeScript types released inside the JS APIs;
- `cargo codegen-schema`, it will update the JSON Schema file of the configuration, used by VSCode;

### Naming patterns for rules

1. Forbid a concept

   ```block
   no<Concept>
   ```

   When a rule's sole intention is to **forbid a single concept** - such as disallowing the use of `debugger` statements - the rule should be named using the `no` prefix. For example, the rule to disallow the use of `debugger` statements is named `noDebugger`.

1. Mandate a concept

   ```block
   use<Concept>
   ```

   When a rule's sole intention is to **mandate a single concept** - such as forcing the use of camel-casing - the rule should be named using the `use` prefix. For example, the rule to mandating the use of camel-cased variable names is named `useCamelCase`.


### `declare_rule`

 This macro is used to declare an analyzer rule type, and implement the
 [RuleMeta] trait for it

 # Example

 The macro itself expect the following syntax:
 ```rust,ignore
use rome_analyze::declare_rule;

 declare_rule! {
     /// Documentation
     pub(crate) ExampleRule {
         version: "0.7.0",
         name: "ruleName",
         recommended: false,
     }
 }
 ```

 #### Documentation

 The doc-comment for the rule is mandatory and is used to generate the
 documentation page for the rule on the website.

 Importantly, the tool used to generate those pages also runs tests on the
 code blocks included in the documentation written in languages supported by
 the Rome toolchain (JavaScript, JSX, TypeScript, ...) similar to how
 `rustdoc` generates tests from code blocks written in Rust. Because code
 blocks in Rust doc-comments are assumed to be written in Rust by default
 the language of the test must be explicitly specified, for instance:

 ```rust,ignore
use rome_analyze::declare_rule;
declare_rule! {
     /// Disallow the use of `var`
     ///
     /// ### Valid
     ///
     /// ```js
     /// let a, b;
     /// ```
     pub(crate) NoVar {
         version: "0.7.0",
         name: "noVar",
         recommended: false,
     }
}
 ```

 Additionally, it's possible to declare that a test should emit a diagnostic
 by adding `expect_diagnostic` to the language metadata:

 ```rust,ignore
use rome_analyze::declare_rule;
 declare_rule! {
     ///  Disallow the use of `var`
     /// 
     ///  ### Invalid
     /// 
     ///  ```js,expect_diagnostic
     ///  var a, b;
     ///  ```
     pub(crate) NoVar {
         version: "0.7.0",
         name: "noVar",
         recommended: false,
     }
 }
 ```

 This will cause the documentation generator to ensure the rule does emit
 exactly one diagnostic for this code, and to include a snapshot for the
 diagnostic in the resulting documentation page

#### Deprecation

 There are occasions when a rule must be deprecated, to avoid breaking changes. The reason
 of deprecations can be multiples.

 In order to do, the macro allows to add additional field to add the reason for deprecation

 ```rust,ignore
use rome_analyze::declare_rule;

 declare_rule! {
      /// Disallow the use of `var`
      /// 
      /// ### Invalid
      /// 
      /// ```js,expect_diagnostic
      /// var a, b;
      /// ```
     pub(crate) NoVar {
         version: "0.7.0",
         name: "noVar",
         deprecated: "Use the rule `noAnotherVar`",
         recommended: false,
     }
 }
 ```

#### Category Macro

 Declaring a rule using `declare_rule!` will cause a new `rule_category!`
 macro to be declared in the surrounding module. This macro can be used to
 refer to the corresponding diagnostic category for this lint rule, if it
 has one. Using this macro instead of getting the category for a diagnostic
 by dynamically parsing its string name has the advantage of statically
 injecting the category at compile time and checking that it is correctly
 registered to the `rome_diagnostics` library

 ```rust,ignore
 declare_rule! {
     /// Documentation
     pub(crate) ExampleRule {
         version: "0.7.0",
         name: "ruleName"
     }
 }

 impl Rule for ExampleRule {
     fn diagnostic(ctx: &RuleContext<Self>, _state: &Self::State) -> Option<RuleDiagnostic> {
         Some(RuleDiagnostic::new(
             rule_category!(),
             ctx.query().text_trimmed_range(),
             "message",
         ))
     }
 }
 ```
