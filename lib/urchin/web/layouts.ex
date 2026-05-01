defmodule Urchin.Web.Layouts do
  use Phoenix.Component

  def root(assigns) do
    ~H"""
    <!DOCTYPE html>
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>urchin · {assigns[:page_title] || "dashboard"}</title>
        <link rel="stylesheet" href="/dashboard.css" />
        <script defer src="/assets/phoenix/phoenix.min.js"></script>
        <script defer src="/assets/phoenix_html/phoenix_html.js"></script>
        <script defer src="/assets/phoenix_live_view/phoenix_live_view.min.js"></script>
        <script defer src="/dashboard.js"></script>
      </head>
      <body>
        {@inner_content}
      </body>
    </html>
    """
  end
end
