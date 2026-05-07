pub fn generate_with_template(my_content: impl Fn() -> String) -> String {
    let my_content = my_content();
    return format!(
        r###"<html><head><title>Authentication</title>
        <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-QWTKZyjpPEjISv5WaRU9OFeRpok6YctnYmDr5pNlyT2bRjXh0JMhjY6hW+ALEwIH" crossorigin="anonymous">
        <style>
        #main {{
            --total-width: 100vw;
            --total-height: 100vh;
            text-align: center;
            width: 100%;
            height: 100vh;
        }}
        </style>
        </head><body>
        <table id="main">
        <tr>
        <td>
        {my_content}
        </td>
        </tr>
        </table>
        
        </body>
        <script>
        </script>
        </html>"###,
    );
}
