import unittest

from tools import lab, test_environment_render_parity as environment, test_recipe_book_render_parity as recipe_book


def row(text):
    return lab.exact_json(text)


class RecipeBookTests(unittest.TestCase):
    def test_a_blits_uv_compares_at_binary32_width(self):
        vanilla = row('{"type": "blit", "u": 0.0, "v": 0.5, "m": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]}')
        sunrise = row('{"type": "blit", "u": -0.0, "v": 0.5, "m": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]}')
        self.assertTrue(vanilla['u'] == sunrise['u'], "Python equality can't tell 0.0 from -0.0")
        self.assertFalse(recipe_book.same(vanilla, sunrise))
        self.assertTrue(recipe_book.same(vanilla, row('{"type": "blit", "u": 0, "v": 0.5, '
                                                     '"m": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]}')))


class EnvironmentTests(unittest.TestCase):
    def full(self):
        return {key: 0.5 for key in environment.MODELED}

    def test_a_row_without_the_modeled_attributes_fails(self):
        vanilla = [{'case': 'overworld 0 0 0 plains', **self.full()}]
        bad, unmodeled = environment.compare_rows(vanilla, [{'case': 'overworld 0 0 0 plains'}])
        self.assertEqual(set(bad), set(environment.MODELED))
        self.assertEqual(unmodeled, set())

    def test_vanilla_only_attributes_are_reported_not_failed(self):
        vanilla = [{'case': 'c', **self.full(), 'minecraft:visual/water_fog_color': 3}]
        bad, unmodeled = environment.compare_rows(vanilla, [{'case': 'c', **self.full()}])
        self.assertEqual(bad, {})
        self.assertEqual(unmodeled, {'minecraft:visual/water_fog_color'})

    def test_an_attribute_sunrise_prints_outside_modeled_fails(self):
        bad, _ = environment.compare_rows([{'case': 'c', **self.full()}],
                                          [{'case': 'c', **self.full(), 'minecraft:visual/new': 1.0}])
        self.assertEqual(set(bad), {'minecraft:visual/new'})


if __name__ == '__main__':
    unittest.main()
