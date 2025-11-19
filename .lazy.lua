local function filter_clangd(server)
	return server ~= "clangd" and server ~= "rust_analyzer" and server ~= "rust-analyzer"
end


vim.filetype.add {
  pattern = {
    ["/.*%.zng"] = 'zngur',
  },
}

vim.treesitter.language.register('rust', 'zngur')

---@type LazySpec
return {
	{
		"AstroNvim/astrocommunity",
		{ import = "astrocommunity.pack.rust" },
		{ import = "astrocommunity.pack.cpp" },
		{ import = "astrocommunity.pack.toml" },
		{ import = "astrocommunity.pack.cmake" },
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
