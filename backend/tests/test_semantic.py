"""Fase C: análisis semántico. Un caso que pasa y uno que falla por código."""

import pytest

from dbms.builder import build
from dbms.catalog import Catalog, IndexSchema, table_from_ast
from dbms.frontend import parse
from dbms.semantic import check, check_script
from dbms.storage import DataDir

SCHEMA = """
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(20) NOT NULL, edad INT,
                          promedio FLOAT, activo BOOLEAN, ingreso DATE, carne VARCHAR(10) UNIQUE);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30), creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT,
                            PRIMARY KEY (estudiante_id, curso_id),
                            FOREIGN KEY (estudiante_id) REFERENCES estudiantes,
                            FOREIGN KEY (curso_id) REFERENCES cursos);
"""


def make_catalog() -> Catalog:
    c = Catalog()
    for s in build(parse(SCHEMA)):
        c.add_table(table_from_ast(s.body, c))
    c.add_index(IndexSchema("idx_edad", "estudiantes", ["edad"]))
    return c


CATALOG = make_catalog()


def problems(sql: str, catalog: Catalog = CATALOG):
    result = parse(sql)
    assert result.ok, [p.to_dict() for p in result.problems]
    return check_script(build(result), catalog)


def codes(sql: str, catalog: Catalog = CATALOG) -> list[str]:
    return [p.code for p in problems(sql, catalog) if p.level == "err"]


# (código, sentencia correcta, sentencia que debe dar ese código y solo ese)
CASES = [
    ("SEM001", "SELECT * FROM cursos", "SELECT * FROM profesores"),
    ("SEM001", "INSERT INTO cursos VALUES (1, 'BD', 4)", "INSERT INTO profesores VALUES (1)"),
    ("SEM001", "DROP TABLE IF EXISTS profesores", "DROP TABLE profesores"),
    ("SEM002", "CREATE TABLE IF NOT EXISTS cursos (x INT)", "CREATE TABLE cursos (x INT)"),
    ("SEM003", "SELECT nombre FROM cursos", "SELECT apellido FROM cursos"),
    ("SEM003", "UPDATE cursos SET creditos = 3", "UPDATE cursos SET horas = 3"),
    ("SEM003", "CREATE INDEX i ON cursos (creditos)", "CREATE INDEX i ON cursos (horas)"),
    ("SEM003", "SELECT 1", "SELECT edad"),
    ("SEM004", "SELECT e.nombre FROM estudiantes e INNER JOIN cursos c ON c.id = e.id",
               "SELECT nombre FROM estudiantes e INNER JOIN cursos c ON c.id = e.id"),
    ("SEM004", "SELECT * FROM cursos a INNER JOIN cursos b ON a.id = b.id",
               "SELECT * FROM cursos INNER JOIN cursos ON cursos.id = cursos.id"),
    ("SEM005", "SELECT e.nombre FROM estudiantes e", "SELECT x.nombre FROM estudiantes e"),
    ("SEM005", "SELECT e.* FROM estudiantes e", "SELECT estudiantes.* FROM estudiantes e"),
    ("SEM006", "SELECT 1 FROM estudiantes WHERE edad >= 18", "SELECT 1 FROM estudiantes WHERE edad >= 'x'"),
    ("SEM006", "SELECT 1 FROM estudiantes WHERE activo", "SELECT 1 FROM estudiantes WHERE edad"),
    ("SEM006", "SELECT edad + promedio FROM estudiantes", "SELECT edad + nombre FROM estudiantes"),
    ("SEM006", "SELECT 1 FROM estudiantes WHERE ingreso > '2024-01-01'",
               "SELECT 1 FROM estudiantes WHERE ingreso > '2024-13-01'"),
    ("SEM006", "SELECT 1 FROM estudiantes WHERE NOT activo AND edad > 1",
               "SELECT 1 FROM estudiantes WHERE NOT edad"),
    ("SEM006", "SELECT 1 FROM estudiantes WHERE nombre LIKE 'A%'", "SELECT 1 FROM estudiantes WHERE edad LIKE 'A%'"),
    ("SEM006", "SELECT AVG(promedio) FROM estudiantes", "SELECT SUM(nombre) FROM estudiantes"),
    ("SEM007", "INSERT INTO cursos VALUES (1, 'BD', 4)", "INSERT INTO cursos VALUES (1, 'BD')"),
    ("SEM007", "INSERT INTO cursos (id, nombre) VALUES (1, 'BD')", "INSERT INTO cursos (id) VALUES (1, 'BD')"),
    ("SEM008", "INSERT INTO cursos VALUES (1, 'BD', 4)", "INSERT INTO cursos VALUES (1, 'BD', 'cuatro')"),
    ("SEM008", "INSERT INTO estudiantes (id, nombre) VALUES (1, 'Ana')",
               "INSERT INTO estudiantes (id, nombre) VALUES (1, 'Un nombre demasiado largo')"),
    ("SEM008", "INSERT INTO estudiantes (id, nombre, ingreso) VALUES (1, 'A', '2024-02-29')",
               "INSERT INTO estudiantes (id, nombre, ingreso) VALUES (1, 'A', '2023-02-29')"),
    ("SEM008", "UPDATE estudiantes SET promedio = edad", "UPDATE estudiantes SET edad = promedio"),
    ("SEM009", "INSERT INTO estudiantes (id, nombre) VALUES (1, 'Ana')", "INSERT INTO estudiantes (id, nombre) VALUES (1, NULL)"),
    ("SEM009", "INSERT INTO cursos (id) VALUES (1)", "INSERT INTO estudiantes (id) VALUES (1)"),
    ("SEM009", "UPDATE estudiantes SET edad = NULL", "UPDATE estudiantes SET nombre = NULL"),
    ("SEM010", "DROP INDEX idx_edad", "DROP INDEX idx_nada"),
    ("SEM011", "CREATE INDEX idx_nota ON inscripciones (nota)", "CREATE INDEX idx_edad ON estudiantes (edad)"),
    ("SEM012", "SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones)",
               "SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id, curso_id FROM inscripciones)"),
    ("SEM012", "SELECT nombre FROM estudiantes WHERE edad > (SELECT AVG(edad) FROM estudiantes)",
               "SELECT nombre FROM estudiantes WHERE edad > (SELECT * FROM estudiantes)"),
    ("SEM013", "SELECT curso_id, COUNT(*) FROM inscripciones GROUP BY curso_id",
               "SELECT curso_id, nota FROM inscripciones GROUP BY curso_id"),
    ("SEM013", "SELECT COUNT(*) FROM inscripciones", "SELECT nota, COUNT(*) FROM inscripciones"),
    ("SEM013", "SELECT curso_id FROM inscripciones GROUP BY curso_id HAVING AVG(nota) > 60",
               "SELECT curso_id FROM inscripciones WHERE AVG(nota) > 60 GROUP BY curso_id"),
    ("SEM013", "SELECT MAX(nota) FROM inscripciones", "SELECT MAX(COUNT(nota)) FROM inscripciones"),
    ("SEM014", "SELECT COUNT(*) FROM cursos", "SELECT UPPER(nombre) FROM cursos"),
    ("SEM014", "SELECT SUM(creditos) FROM cursos", "SELECT SUM(*) FROM cursos"),
    ("SEM015", "USE principal", "USE tienda"),
    ("SEM016", "CREATE DATABASE tienda", "CREATE DATABASE principal"),
    ("SEM017", "CREATE TABLE t (a INT, b INT)", "CREATE TABLE t (a INT, a INT)"),
    ("SEM017", "INSERT INTO cursos (id, nombre) VALUES (1, 'x')", "INSERT INTO cursos (id, id) VALUES (1, 2)"),
    ("SEM017", "UPDATE cursos SET creditos = 1, nombre = 'x'", "UPDATE cursos SET creditos = 1, creditos = 2"),
    ("SEM018", "CREATE TABLE t (a INT PRIMARY KEY, b INT)", "CREATE TABLE t (a INT PRIMARY KEY, b INT PRIMARY KEY)"),
    ("SEM018", "CREATE TABLE t (c INT REFERENCES cursos)", "CREATE TABLE t (n VARCHAR(30) REFERENCES cursos (nombre))"),
    ("SEM018", "CREATE TABLE t (n VARCHAR(5))", "CREATE TABLE t (n VARCHAR(0))"),
    ("SEM099", "SELECT 1 FROM estudiantes e INNER JOIN inscripciones i ON i.estudiante_id = e.id",
               "SELECT 1 FROM estudiantes e LEFT JOIN inscripciones i ON i.estudiante_id = e.id"),
    ("SEM099", "SHOW TABLES", "ALTER TABLE cursos ADD COLUMN horas INT"),
]


@pytest.mark.parametrize("code, ok, bad", CASES, ids=[f"{c}-{i}" for i, (c, _, _) in enumerate(CASES)])
def test_codigo(code, ok, bad):
    assert codes(ok) == [], problems(ok)
    assert codes(bad) == [code], problems(bad)


def test_ejemplos_del_enunciado_son_correctos():
    assert codes("""
        SELECT nombre FROM estudiantes WHERE edad >= 18;
        SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones WHERE curso_id = 3);
        SELECT e.nombre FROM estudiantes e
            WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id);
        SELECT c.nombre, COUNT(*) AS n, AVG(i.nota) AS prom
            FROM cursos c INNER JOIN inscripciones i ON i.curso_id = c.id
            GROUP BY c.nombre HAVING COUNT(*) >= 2 ORDER BY prom DESC, n;
        UPDATE estudiantes SET edad = edad + 1 WHERE ingreso < '2024-06-01';
        DELETE FROM inscripciones WHERE nota IS NULL OR nota BETWEEN 0 AND 10;
    """) == []


def test_posicion_y_mensaje():
    [p] = problems("SELECT nombre\nFROM estudiantes\nWHERE edad >= 'x';")
    assert (p.code, p.line, p.col) == ("SEM006", 3, 7)
    assert p.msg == "no se puede comparar edad (número) con 'x' (texto)"
    [p] = problems("SELECT x.nombre FROM estudiantes e;")
    assert (p.line, p.col) == (1, 8) and "'x' no es una tabla ni un alias" in p.msg


def test_reporta_todo_sin_detenerse():
    assert codes("SELECT a, b FROM estudiantes WHERE c = 1") == ["SEM003", "SEM003", "SEM003"]


def test_sin_cascada_cuando_falta_la_tabla():
    # Solo la tabla inexistente: sus columnas no generan más errores.
    assert codes("SELECT p.nombre, sueldo FROM profesores p WHERE p.edad > 'x'") == ["SEM001"]


def test_subconsulta_correlacionada_ve_la_consulta_externa():
    assert codes("SELECT nombre FROM estudiantes e WHERE EXISTS "
                 "(SELECT 1 FROM inscripciones WHERE estudiante_id = e.id AND nota > e.promedio)") == []
    # La columna externa sin calificar también se encuentra (no está en inscripciones).
    assert codes("SELECT nombre FROM estudiantes WHERE EXISTS "
                 "(SELECT 1 FROM inscripciones WHERE estudiante_id = id AND nota > promedio)") == []
    # Si está en las dos, gana la interna (no es ambigua): nombre es cursos.nombre.
    assert codes("SELECT nombre FROM estudiantes WHERE EXISTS "
                 "(SELECT 1 FROM cursos WHERE nombre = 'BD')") == []
    # Una columna de la consulta externa no se puede calificar con un alias interno.
    assert codes("SELECT nombre FROM estudiantes e WHERE EXISTS "
                 "(SELECT 1 FROM cursos c WHERE c.edad > 1)") == ["SEM003"]


def test_group_by_acepta_la_misma_columna_calificada_o_no():
    assert codes("SELECT i.curso_id, COUNT(*) FROM inscripciones i GROUP BY curso_id") == []
    assert codes("SELECT creditos * 2, COUNT(*) FROM cursos GROUP BY creditos * 2") == []
    assert codes("SELECT * FROM cursos GROUP BY id") == ["SEM013"]


def test_order_by_usa_alias():
    assert codes("SELECT edad AS e FROM estudiantes ORDER BY e DESC") == []
    assert codes("SELECT edad AS e FROM estudiantes WHERE e > 1") == ["SEM003"]


def test_insert_con_expresiones_y_null():
    assert codes("INSERT INTO cursos VALUES (1 + 1, NULL, -3)") == []
    assert codes("INSERT INTO cursos VALUES (1, 'x', creditos)") == ["SEM003"]
    assert codes("INSERT INTO cursos VALUES (1, 'x', 2), (2, 'y')") == ["SEM007"]


def test_foreign_keys():
    assert codes("CREATE TABLE t (c INT REFERENCES profesores)") == ["SEM001"]
    assert codes("CREATE TABLE t (c INT REFERENCES cursos (horas))") == ["SEM003"]
    assert codes("CREATE TABLE t (c VARCHAR(3) REFERENCES cursos)") == ["SEM006"]
    assert codes("CREATE TABLE t (a INT, b INT, FOREIGN KEY (a, b) REFERENCES cursos)") == ["SEM018"]
    assert codes("CREATE TABLE emp (id INT PRIMARY KEY, jefe INT REFERENCES emp)") == []
    assert codes("CREATE TABLE t (a INT, PRIMARY KEY (b))") == ["SEM003"]


def test_check_y_default():
    assert codes("CREATE TABLE t (n INT, CHECK (n > 0))") == []
    assert codes("CREATE TABLE t (n INT, CHECK (n))") == ["SEM006"]
    assert codes("CREATE TABLE t (n INT, CHECK (m > 0))") == ["SEM003"]
    [p] = problems("CREATE TABLE t (n INT DEFAULT 0)")
    assert (p.level, p.code) == ("warn", "SEM099")


def test_script_aplica_los_efectos_en_orden():
    assert codes("""
        CREATE TABLE profesores (id INT PRIMARY KEY, nombre VARCHAR(10));
        INSERT INTO profesores VALUES (1, 'Ada');
        CREATE INDEX idx_p ON profesores (nombre);
        DROP INDEX idx_p;
        DROP TABLE profesores;
        SELECT * FROM profesores;
    """) == ["SEM001"]
    # Una sentencia con error no tiene efecto: la tabla no llega a existir.
    assert codes("CREATE TABLE x (a INT, a INT); SELECT * FROM x;") == ["SEM017", "SEM001"]
    assert codes("CREATE TABLE cursos2 (id INT); CREATE TABLE cursos2 (id INT);") == ["SEM002"]


def test_script_no_modifica_el_catalogo_original():
    c = make_catalog()
    problems("DROP TABLE cursos; CREATE TABLE nueva (a INT);", c)
    assert c.table("cursos") is not None and c.table("nueva") is None


def test_check_de_una_sentencia():
    [stmt] = build(parse("SELECT nada FROM cursos"))
    assert [p.code for p in check(stmt, CATALOG)] == ["SEM003"]


def _workspace(name: str) -> str:
    from pathlib import Path
    return (Path(__file__).resolve().parents[2] / "workspace" / name).read_text(encoding="utf-8")


def test_demo_no_tiene_problemas():
    assert problems(_workspace("demo.sql"), Catalog()) == []


def test_errores_semanticos_sql_da_el_codigo_de_cada_linea():
    """Cada línea marcada `-- SEMxxx` en workspace/entrega3_errores_semanticos.sql
    produce exactamente ese código, en esa línea."""
    import re
    text = _workspace("entrega3_errores_semanticos.sql")
    expected = [(i, m.group(1)) for i, line in enumerate(text.splitlines(), start=1)
                if (m := re.search(r"-- (SEM\d{3})", line))]
    assert [(p.line, p.code) for p in problems(text, Catalog())] == expected


def test_use_y_bases_de_datos(tmp_path):
    data = DataDir(tmp_path)
    default = data.open(create=True)
    tienda = data.create("tienda")
    c = make_catalog()
    tienda.save_catalog(c)
    script = build(parse("""
        USE tienda;
        SELECT nombre FROM cursos;
        CREATE DATABASE nueva;
        USE nueva;
        SELECT * FROM cursos;
        DROP DATABASE nueva;
        USE nueva;
    """))
    found = check_script(script, default.load_catalog(), data)
    assert [p.code for p in found] == ["SEM001", "SEM015"]
    assert [p.line for p in found] == [6, 8]
