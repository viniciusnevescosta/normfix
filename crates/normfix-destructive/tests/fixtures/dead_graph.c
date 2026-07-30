static void	dead_leaf(void);

static void	dead_leaf(void)
{
}

static void	dead_root(void)
{
	dead_leaf();
}

static void	live_leaf(void)
{
}

void	public_api(void)
{
	live_leaf();
}
