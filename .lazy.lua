local function filter_clangd(server)
	return server ~= "clangd" and server ~= "rust_analyzer" and server ~= "rust-analyzer"
end

---@type LazySpec
return {
	{
		"AstroNvim/astrocommunity",
		{ import = "astrocommunity.pack.rust" },
		{ import = "astrocommunity.pack.cpp" },
	},
	{
		"WhoIsSethDaniel/mason-tool-installer.nvim",
		-- overrides `require("mason-lspconfig").setup(...)`
		opts = function(_, opts)
			-- mason clangd doesn't work on nix
			opts.ensure_installed = vim.tbl_filter(filter_clangd, opts.ensure_installed)
		end,
	},
	{
		"williamboman/mason-lspconfig.nvim",
		enabled = false,
		opts = function(_, opts)
			-- mason clangd doesn't work on nix
			opts.ensure_installed = vim.tbl_filter(filter_clangd, opts.ensure_installed)
		end,
	},
}
